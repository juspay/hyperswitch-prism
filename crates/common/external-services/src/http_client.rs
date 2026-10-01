use std::error::Error as StdError;

use common_enums::ApiClientError;
use error_stack::{report, Report};

/// Sends an HTTP request via reqwest, automatically retrying once if the
/// connection was closed by the remote side before the message completed.
///
/// This handles a well-known race condition in HTTP connection pooling: hyper
/// maintains a pool of idle keep-alive connections and occasionally selects one
/// at the exact moment the remote server sends a TCP FIN to close it. hyper
/// writes the request head, transitions the connection to Busy, then gets EOF
/// on the read side — producing `is_incomplete_message()`.
///
/// Technically, the request bytes may have reached the server (TCP FIN is a
/// half-close — the server's recv buffer can still accept data). However, in
/// practice idle-connection cleanup uses `close()` (not `shutdown(SHUT_WR)`),
/// which closes both directions and RSTs incoming data. This is why
/// Hyperswitch has run this same blind retry in production at scale without
/// double-charge incidents.
///
/// The request is cloned via `try_clone()` before the first attempt. If the
/// body is a stream that cannot be cloned, the retry is skipped and the
/// original error is returned.
///
/// Returns `(response, retried)` where `retried` is `true` if the first attempt
/// hit a connection-closed error and a retry was attempted.
///
/// The retry is done on the client returned by `make_retry_client`, evaluated
/// lazily only when the retry is needed. Pass a client with a brand-new
/// connection pool (see `create_fresh_client` in `service.rs`) so the retry
/// dials a NEW TCP connection; reusing the same client risks checking out
/// another equally stale keep-alive connection from the shared pool and losing
/// the same race again. If the factory fails, the retry falls back to the
/// already-created client that made the first attempt.
pub async fn send_request_with_retry(
    request: reqwest::RequestBuilder,
    make_retry_client: impl FnOnce() -> Result<reqwest::Client, Report<ApiClientError>>,
) -> (Result<reqwest::Response, Report<ApiClientError>>, bool) {
    // Clone the request before sending so we have a backup for retry.
    // `try_clone()` returns None for streaming bodies that cannot be cloned.
    let cloned_request = request.try_clone();

    let response = send_request(request).await;

    match response {
        Err(ref error)
            if error.current_context() == &ApiClientError::ConnectionClosedIncompleteMessage =>
        {
            match cloned_request {
                Some(cloned_req) => match make_retry_client() {
                    Ok(retry_client) => {
                        tracing::info!(
                            "Retrying request on a fresh client (new connection) due to connection closed before message completed"
                        );
                        (
                            send_request_with_client(&retry_client, cloned_req).await,
                            true,
                        )
                    }
                    Err(client_error) => {
                        // Fall back to retrying on the already-created client
                        // (shared pool) rather than failing outright.
                        tracing::warn!(
                            ?client_error,
                            "Cannot build fresh retry client; retrying on the existing client"
                        );
                        (send_request(cloned_req).await, true)
                    }
                },
                None => {
                    tracing::info!(
                        "Cannot retry connection-closed error: request body is not cloneable"
                    );
                    (response, false)
                }
            }
        }
        response => (response, false),
    }
}

/// Sends a single HTTP request and maps reqwest errors to `ApiClientError`.
async fn send_request(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, Report<ApiClientError>> {
    request.send().await.map_err(map_reqwest_error)
}

/// Builds the request and sends it through the given (fresh) client, mapping
/// reqwest errors to `ApiClientError`.
async fn send_request_with_client(
    client: &reqwest::Client,
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, Report<ApiClientError>> {
    // Static message: `build()` errors can embed header values (auth
    // signatures, cert-derived material); never string them into the report.
    let request = request.build().map_err(|_error| {
        report!(ApiClientError::RequestNotSent(
            "failed to build connector request".to_string()
        ))
    })?;
    client.execute(request).await.map_err(map_reqwest_error)
}

fn map_reqwest_error(error: reqwest::Error) -> Report<ApiClientError> {
    let api_error = match error {
        error if error.is_timeout() => ApiClientError::RequestTimeoutReceived,
        error if is_connection_closed_before_message_could_complete(&error) => {
            ApiClientError::ConnectionClosedIncompleteMessage
        }
        // Never string-passthrough the error: reqwest's message can embed
        // request content (headers, query strings, cert-derived material).
        // Report only a static classification by error kind.
        error if error.is_connect() => ApiClientError::RequestNotSent("connect error".to_string()),
        error if error.is_builder() => {
            ApiClientError::RequestNotSent("client build error".to_string())
        }
        error if error.is_request() => ApiClientError::RequestNotSent("request error".to_string()),
        error if error.is_redirect() => {
            ApiClientError::RequestNotSent("redirect error".to_string())
        }
        _ => ApiClientError::RequestNotSent("request could not be sent".to_string()),
    };
    report!(api_error)
}

/// Checks whether a `reqwest::Error` was caused by hyper's
/// "connection closed before message completed" condition.
///
/// This walks the error source chain to find the underlying `hyper::Error`
/// and checks `is_incomplete_message()`. This error is raised on the read
/// side — hyper wrote the request and got EOF while reading the response.
/// It typically occurs when hyper's connection pool selects a stale idle
/// connection that the remote server is concurrently closing.
/// Count of this host's TCP sockets grouped by connection state.
///
/// `ESTABLISHED` sockets with no in-flight request are exactly the pool's idle
/// keep-alive population; `CLOSE_WAIT` sockets are peers that already sent a
/// FIN which the pool hasn't reaped yet — the armed stale-connection landmines.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct TcpSocketSnapshot {
    established: u64,
    close_wait: u64,
    time_wait: u64,
    fin_wait: u64,
    other: u64,
}

impl TcpSocketSnapshot {
    fn record(&mut self, state: &str) {
        match state {
            "01" => self.established += 1,         // ESTABLISHED
            "04" | "05" => self.fin_wait += 1,     // FIN_WAIT1/2
            "06" => self.time_wait += 1,           // TIME_WAIT
            "08" => self.close_wait += 1,          // CLOSE_WAIT
            "02" | "03" | "09" => self.other += 1, // SYN_SENT/SYN_RECV/LAST_ACK
            _ => {}                                // skip CLOSE / CLOSING
        }
    }
}

/// Best-effort snapshot of this host's TCP sockets from `/proc/net/tcp{,6}`.
///
/// Returns `(filtered, total)`: `filtered` counts only sockets whose REMOTE
/// port matches `remote_port` (outbound sockets toward the connector/proxy
/// endpoint — close as we can get to "pool connections for this host"), `total`
/// counts every realized outbound socket. `None` if the table is unreadable.
#[cfg(target_os = "linux")]
fn tcp_socket_snapshot(remote_port: Option<u16>) -> Option<(TcpSocketSnapshot, TcpSocketSnapshot)> {
    let mut filtered = TcpSocketSnapshot::default();
    let mut total = TcpSocketSnapshot::default();
    let mut read_any = false;

    for path in ["/proc/net/tcp", "/proc/net/tcp6"] {
        let Ok(contents) = std::fs::read_to_string(path) else {
            continue;
        };
        read_any = true;
        for line in contents.lines().skip(1) {
            // Format: sl local_address rem_address st ...
            let mut fields = line.split_whitespace();
            let _sl = fields.next();
            let _local = fields.next();
            let (Some(remote), Some(state)) = (fields.next(), fields.next()) else {
                continue;
            };
            if state == "0A" {
                // LISTEN: inbound, never part of a client pool
                continue;
            }
            total.record(state);

            let port_matches = remote
                .rsplit(':')
                .next()
                .and_then(|port| u16::from_str_radix(port, 16).ok())
                .is_some_and(|port| remote_port.is_none_or(|want| port == want));
            if port_matches {
                filtered.record(state);
            }
        }
    }

    read_any.then_some((filtered, total))
}

#[cfg(not(target_os = "linux"))]
fn tcp_socket_snapshot(
    _remote_port: Option<u16>,
) -> Option<(TcpSocketSnapshot, TcpSocketSnapshot)> {
    None
}

/// Logs this host's idle/dead TCP socket counts for the connector's port at
/// the moment a retry fired — `filtered_*` ≈ outbound sockets toward this
/// connector's endpoint, `total_*` = the whole host. `CLOSE_WAIT` counts how
/// many pooled connections the upstream had already FINed (armed landmines).
pub(crate) fn log_tcp_socket_snapshot(connector: &str, remote_port: Option<u16>) {
    match tcp_socket_snapshot(remote_port) {
        Some((filtered, total)) => {
            tracing::info!(
                connector,
                remote_port,
                filtered_established = filtered.established,
                filtered_close_wait = filtered.close_wait,
                filtered_time_wait = filtered.time_wait,
                filtered_fin_wait = filtered.fin_wait,
                total_established = total.established,
                total_close_wait = total.close_wait,
                total_time_wait = total.time_wait,
                total_fin_wait = total.fin_wait,
                "TCP socket snapshot at auto-retry (stale pooled-connection race)"
            );
        }
        None => {
            tracing::info!(
                connector,
                "TCP socket snapshot unavailable (cannot read /proc/net/tcp*)"
            );
        }
    }
}

fn is_connection_closed_before_message_could_complete(error: &reqwest::Error) -> bool {
    let mut source = error.source();
    while let Some(err) = source {
        if let Some(hyper_err) = err.downcast_ref::<hyper::Error>() {
            if hyper_err.is_incomplete_message() {
                return true;
            }
        }
        source = err.source();
    }
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    /// Local server that closes the first `close_first` connections without answering
    /// (what hyper sees when it picks a stale keep-alive connection), then replies 200.
    async fn spawn_server(close_first: usize) -> (String, Arc<AtomicUsize>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/", listener.local_addr().unwrap());
        let seen = Arc::new(AtomicUsize::new(0));
        let counter = seen.clone();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let n = counter.fetch_add(1, Ordering::SeqCst);
                tokio::spawn(async move {
                    let mut buf = [0u8; 2048];
                    let _ = stream.read(&mut buf).await;
                    if n >= close_first {
                        let _ = stream
                            .write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                            )
                            .await;
                    }
                });
            }
        });
        (url, seen)
    }

    #[tokio::test]
    async fn detects_connection_closed_before_message_completed() {
        let (url, _) = spawn_server(usize::MAX).await;
        let error = reqwest::Client::new().get(&url).send().await.unwrap_err();
        assert!(is_connection_closed_before_message_could_complete(&error));
    }

    #[tokio::test]
    async fn retries_once_when_connection_closed_before_message_completed() {
        let (url, seen) = spawn_server(1).await;
        let (response, retried) = send_request_with_retry(reqwest::Client::new().get(&url), || {
            Ok(reqwest::Client::new())
        })
        .await;
        assert!(retried, "the request should have been retried");
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retries_on_existing_client_when_fresh_retry_client_cannot_be_built() {
        let (url, seen) = spawn_server(usize::MAX).await;
        let (response, retried) = send_request_with_retry(reqwest::Client::new().get(&url), || {
            Err(report!(ApiClientError::ClientConstructionFailed))
        })
        .await;
        assert!(
            retried,
            "the request should have been retried on the existing client"
        );
        assert_eq!(
            response.unwrap_err().current_context(),
            &ApiClientError::ConnectionClosedIncompleteMessage
        );
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retry_client_factory_is_called_exactly_once_on_failure() {
        let (url, _) = spawn_server(1).await;
        let factory_called = Arc::new(AtomicUsize::new(0));
        let counter = factory_called.clone();
        let (response, retried) = send_request_with_retry(reqwest::Client::new().get(&url), || {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(reqwest::Client::new())
        })
        .await;
        assert!(retried);
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(factory_called.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn retry_client_factory_is_not_called_when_first_attempt_succeeds() {
        let (url, _) = spawn_server(0).await;
        let factory_called = Arc::new(AtomicUsize::new(0));
        let counter = factory_called.clone();
        let (response, retried) = send_request_with_retry(reqwest::Client::new().get(&url), || {
            counter.fetch_add(1, Ordering::SeqCst);
            Ok(reqwest::Client::new())
        })
        .await;
        assert!(!retried);
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(
            factory_called.load(Ordering::SeqCst),
            0,
            "the fresh client must only be built when a retry is actually needed"
        );
    }

    #[tokio::test]
    async fn connect_error_maps_to_static_string_without_raw_error_text() {
        // Port 9 (discard) is not listening: connect refused every time.
        let error = reqwest::Client::new()
            .get("http://127.0.0.1:9/")
            .send()
            .await
            .unwrap_err();
        let mapped = map_reqwest_error(error);
        assert_eq!(
            mapped.current_context(),
            &ApiClientError::RequestNotSent("connect error".to_string())
        );
        let debug = format!("{:?}", mapped);
        assert!(
            !debug.contains("refused") && !debug.contains("http://"),
            "raw error text/URL must never flow into the report: {debug}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn tcp_socket_snapshot_reads_proc_table() {
        let (filtered, total) =
            tcp_socket_snapshot(Some(443)).expect("/proc/net/tcp should be readable on linux");
        assert!(filtered.established <= total.established);
        assert!(filtered.close_wait <= total.close_wait);
        let unfiltered = tcp_socket_snapshot(None).expect("unfiltered snapshot");
        assert_eq!(unfiltered.1, total);
    }

    #[tokio::test]
    async fn does_not_retry_more_than_once() {
        let (url, seen) = spawn_server(usize::MAX).await;
        let (response, retried) = send_request_with_retry(reqwest::Client::new().get(&url), || {
            Ok(reqwest::Client::new())
        })
        .await;
        assert!(retried);
        assert_eq!(
            response.unwrap_err().current_context(),
            &ApiClientError::ConnectionClosedIncompleteMessage
        );
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }
}
