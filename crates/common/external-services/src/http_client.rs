use std::error::Error as StdError;

use common_enums::ApiClientError;
use error_stack::{report, Report};

/// Sends an HTTP request via reqwest, automatically retrying once if the
/// connection was closed by the remote side before the message could complete.
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
/// The retry is sent through the client returned by `retry_client_factory`,
/// falling back to the original `client` if the factory yields `None`. The
/// factory is evaluated lazily — only when the first attempt actually hits a
/// connection-closed error — so the hot path carries zero retry-client cost.
/// Callers SHOULD pass a factory that produces a dedicated client whose pool
/// never retains idle connections (`pool_max_idle_per_host(0)` — see
/// `create_retry_client` in `service.rs`): retrying on the same client that
/// just lost the race hands the retry to the very same pool, which during a
/// bursty traffic pattern is likely to hold other equally stale connections —
/// the retry then loses the same race again on a second corpse.
///
/// When the connection-closed error fires, a best-effort snapshot of this
/// host's TCP socket table (from `/proc/net/tcp*`) is logged so operators can
/// correlate failures with the number of idle/half-closed sockets on the machine.
///
/// The request is cloned via `try_clone()` before the first attempt. If the
/// body is a stream that cannot be cloned, the retry is skipped and the
/// original error is returned.
///
/// Returns `(response, retried)` where `retried` is `true` if the first attempt
/// hit a connection-closed error and a retry was attempted.
pub async fn send_request_with_retry(
    client: &reqwest::Client,
    request: reqwest::Request,
    retry_client_factory: impl FnOnce() -> Option<reqwest::Client>,
) -> (Result<reqwest::Response, Report<ApiClientError>>, bool) {
    let request_url = request.url().clone();
    // Clone the request before sending so we have a backup for retry.
    // `try_clone()` returns None for streaming bodies that cannot be cloned.
    let cloned_request = request.try_clone();

    let response = execute_request(client, request).await;

    match response {
        Err(ref error)
            if error.current_context() == &ApiClientError::ConnectionClosedIncompleteMessage =>
        {
            log_tcp_socket_snapshot(&request_url);
            match cloned_request {
                Some(cloned) => {
                    let retry_client = retry_client_factory();
                    let retry_client = retry_client.as_ref().unwrap_or(client);
                    tracing::info!(
                        url = %request_url,
                        "Retrying request due to connection closed before message completed on a fresh connection"
                    );
                    (execute_request(retry_client, cloned).await, true)
                }
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
async fn execute_request(
    client: &reqwest::Client,
    request: reqwest::Request,
) -> Result<reqwest::Response, Report<ApiClientError>> {
    client.execute(request).await.map_err(|error| {
        let api_error = match error {
            error if error.is_timeout() => ApiClientError::RequestTimeoutReceived,
            error if is_connection_closed_before_message_could_complete(&error) => {
                ApiClientError::ConnectionClosedIncompleteMessage
            }
            // Strip the URL so credentials carried in the query string never
            // reach the error logs.
            _ => ApiClientError::RequestNotSent(error.without_url().to_string()),
        };
        report!(api_error)
    })
}

/// Checks whether a `reqwest::Error` was caused by hyper's
/// "connection closed before message completed" condition.
///
/// This walks the error source chain to find the underlying `hyper::Error`
/// and checks `is_incomplete_message()`. This error is raised on the read
/// side — hyper wrote the request and got EOF while reading the response.
/// It typically occurs when hyper's connection pool selects a stale idle
/// connection that the remote server is concurrently closing.
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

/// Count of this host's TCP sockets grouped by connection state.
///
/// `ESTABLISHED` sockets with no in-flight request are exactly the idle
/// keep-alive connections hyper's connection pool hands out on checkout.
/// `CLOSE_WAIT` sockets are peers that sent us a FIN which the pool has not
/// reaped yet — the "armed landmine" population. A high `close_wait` count at
/// the moment of a connection-closed error is direct evidence for the
/// stale-pool race this module exists to survive.
#[derive(Debug, Default, Clone, Copy)]
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
            _ => {}                                // skip CLOSE / CLOSING / LISTEN — not actionable
        }
    }
}

/// Best-effort snapshot of this host's TCP socket table from `/proc/net/tcp{,6}`.
///
/// Returns `(filtered, total)` where `filtered` only counts sockets whose
/// remote port equals `remote_port` (the connector/upstream port, so the
/// filtered counts track sockets that could end up in this connector's pool)
/// and `total` counts every realised outbound socket on the host. Returns
/// `None` when the table cannot be read (restricted procfs etc.).
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

/// `/proc/net/tcp*` is Linux-only; nothing actionable to report elsewhere.
#[cfg(not(target_os = "linux"))]
fn tcp_socket_snapshot(
    _remote_port: Option<u16>,
) -> Option<(TcpSocketSnapshot, TcpSocketSnapshot)> {
    None
}

/// Logs the current TCP-socket table broken down by state, so a
/// connection-closed failure can be correlated with the number of idle
/// (`ESTABLISHED`) and half-closed (`CLOSE_WAIT`) sockets on the host.
fn log_tcp_socket_snapshot(request_url: &reqwest::Url) {
    let remote_port = request_url.port_or_known_default();
    match tcp_socket_snapshot(remote_port) {
        Some((filtered, total)) => {
            tracing::info!(
                url = %request_url,
                remote_port,
                filtered_established = filtered.established,
                filtered_close_wait = filtered.close_wait,
                filtered_time_wait = filtered.time_wait,
                filtered_fin_wait = filtered.fin_wait,
                total_established = total.established,
                total_close_wait = total.close_wait,
                total_time_wait = total.time_wait,
                total_fin_wait = total.fin_wait,
                "TCP socket snapshot at connection-closed error (stale idle-pool connection race)"
            );
        }
        None => {
            tracing::info!(
                url = %request_url,
                "TCP socket snapshot unavailable (cannot read /proc/net/tcp*)"
            );
        }
    }
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
        let client = reqwest::Client::new();
        let request = client.get(&url).build().unwrap();
        let (response, retried) = send_request_with_retry(&client, request, || None).await;
        assert!(retried, "the request should have been retried");
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retries_on_dedicated_retry_client_when_provided() {
        // The dedicated retry client never retains idle connections, so the
        // retry is guaranteed to dial a brand-new connection instead of
        // checking out another potentially stale one from the shared pool.
        let (url, seen) = spawn_server(1).await;
        let client = reqwest::Client::new();
        let retry_client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();
        let request = client.get(&url).build().unwrap();
        let (response, retried) =
            send_request_with_retry(&client, request, || Some(retry_client)).await;
        assert!(retried, "the request should have been retried");
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn retry_client_factory_is_not_called_when_first_attempt_succeeds() {
        let (url, _) = spawn_server(0).await;
        let client = reqwest::Client::new();
        let request = client.get(&url).build().unwrap();
        let factory_called = Arc::new(AtomicUsize::new(0));
        let counter = factory_called.clone();
        let (response, retried) = send_request_with_retry(&client, request, || {
            counter.fetch_add(1, Ordering::SeqCst);
            None
        })
        .await;
        assert!(!retried);
        assert_eq!(response.unwrap().status(), 200);
        assert_eq!(
            factory_called.load(Ordering::SeqCst),
            0,
            "the factory must only run when a retry is actually needed"
        );
    }

    #[tokio::test]
    async fn does_not_retry_more_than_once() {
        let (url, seen) = spawn_server(usize::MAX).await;
        let client = reqwest::Client::new();
        let retry_client = reqwest::Client::builder()
            .pool_max_idle_per_host(0)
            .build()
            .unwrap();
        let request = client.get(&url).build().unwrap();
        let (response, retried) =
            send_request_with_retry(&client, request, || Some(retry_client)).await;
        assert!(retried);
        assert_eq!(
            response.unwrap_err().current_context(),
            &ApiClientError::ConnectionClosedIncompleteMessage
        );
        assert_eq!(seen.load(Ordering::SeqCst), 2);
    }

    #[cfg(target_os = "linux")]
    #[tokio::test]
    async fn tcp_socket_snapshot_reads_proc_table() {
        let (url, _) = spawn_server(usize::MAX).await;
        let client = reqwest::Client::new();
        let request = client.get(&url).build().unwrap();
        let remote_port = request.url().port_or_known_default();
        let (filtered, total) = tcp_socket_snapshot(remote_port)
            .expect("should be able to read /proc/net/tcp on linux");
        assert!(filtered.established <= total.established);
        assert!(filtered.close_wait <= total.close_wait);
    }
}
