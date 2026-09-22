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
pub async fn send_request_with_retry(
    request: reqwest::RequestBuilder,
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
                Some(cloned) => {
                    tracing::info!(
                        "Retrying request due to connection closed before message completed"
                    );
                    (send_request(cloned).await, true)
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
async fn send_request(
    request: reqwest::RequestBuilder,
) -> Result<reqwest::Response, Report<ApiClientError>> {
    request.send().await.map_err(|error| {
        let api_error = match error {
            error if error.is_timeout() => ApiClientError::RequestTimeoutReceived,
            error if is_connection_closed_before_message_could_complete(&error) => {
                ApiClientError::ConnectionClosedIncompleteMessage
            }
            _ => ApiClientError::RequestNotSent(error.to_string()),
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
