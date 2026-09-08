use std::error::Error as StdError;

use common_enums::ApiClientError;
use error_stack::{report, Report};

/// Sends an HTTP request via reqwest, automatically retrying once if the
/// connection was closed by the remote side before the message completed.
///
/// This handles a well-known race condition in HTTP connection pooling: hyper
/// maintains a pool of idle keep-alive connections and occasionally selects one
/// at the exact moment the remote server sends a TCP FIN to close it. Since
/// hyper has already begun writing the request bytes, it cannot silently retry
/// — the server may have acted on partial data. However, the
/// `is_incomplete_message()` error tells us the full HTTP request was never
/// transmitted, making it safe to retry exactly once on a fresh connection.
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
/// and checks `is_incomplete_message()`. This error occurs when hyper's
/// connection pool selects an idle connection at the same moment the remote
/// server sends a TCP FIN to close it.
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
