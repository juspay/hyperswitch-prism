use std::error::Error as StdError;

use common_enums::ApiClientError;
use error_stack::{report, Report};

const MAX_ERROR_SOURCE_DEPTH: usize = 8;

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
                            retry_reason = %error.current_context(),
                            "Retrying request once on a fresh client (new connection)"
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
                            client_error = %client_error.current_context(),
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
    // Inspect typed causes before mapping discards them. Never log their raw
    // text: URLs, custom I/O errors and TLS errors can contain request secrets.
    let (mut channel_closed, mut canceled, mut incomplete_message) = (false, false, false);
    let (mut io_kind, mut os_error) = (None, None);
    let mut error_sources = Vec::new();
    let mut source = error.source();
    for depth in 0..MAX_ERROR_SOURCE_DEPTH {
        let Some(cause) = source else { break };
        if let Some(cause) = cause.downcast_ref::<hyper::Error>() {
            channel_closed |= cause.is_closed();
            canceled |= cause.is_canceled();
            incomplete_message |= cause.is_incomplete_message();
        }
        if let Some(cause) = cause.downcast_ref::<std::io::Error>() {
            io_kind = Some(cause.kind());
            os_error = cause.raw_os_error();
        }
        let (kind, message, details_omitted) = safe_transport_cause(cause);
        error_sources.push(serde_json::json!({
            "depth": depth,
            "kind": kind,
            "message": message,
            "details_omitted": details_omitted,
        }));
        // io::Error::source() can skip its immediate custom payload.
        source = cause
            .downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::get_ref)
            .map(|inner| -> &(dyn StdError + 'static) { inner })
            .or_else(|| cause.source());
    }
    tracing::warn!(
        connect_error = error.is_connect(),
        request_error = error.is_request(),
        timeout = error.is_timeout(),
        channel_closed,
        canceled,
        incomplete_message,
        ?io_kind,
        ?os_error,
        error_sources = %serde_json::json!(error_sources),
        source_chain_truncated = source.is_some(),
        "Connector HTTP request failed"
    );
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

// Hyper 1.8.1 and hyper-util 0.1.20 Display print fixed kind messages only.
// Update the pinned versions and message tests together.
// Inspect known types only. Arbitrary Display/Debug output may contain URLs,
// credentials, request data, or certificate names. Omitted causes stay visible.
fn safe_transport_cause(cause: &(dyn StdError + 'static)) -> (&'static str, String, bool) {
    if let Some(error) = cause.downcast_ref::<hyper::Error>() {
        return ("hyper", error.to_string(), false);
    }
    if let Some(error) = cause.downcast_ref::<hyper_util::client::legacy::Error>() {
        return ("hyper_util", error.to_string(), false);
    }
    if let Some(error) = cause.downcast_ref::<std::io::Error>() {
        return match error.raw_os_error() {
            Some(code) => (
                "io",
                std::io::Error::from_raw_os_error(code).to_string(),
                false,
            ),
            None => (
                "io",
                format!("I/O error: {:?}", error.kind()),
                error.get_ref().is_some(),
            ),
        };
    }
    if cause.is::<native_tls::Error>() {
        return (
            "native_tls",
            "native TLS error; backend details omitted here".to_string(),
            true,
        );
    }
    if let Some(error) = cause.downcast_ref::<rustls::Error>() {
        let (message, omitted) = rustls_message(error);
        return ("rustls", message, omitted);
    }
    #[cfg(target_os = "linux")]
    if let Some(message) = safe_openssl_cause(cause) {
        return ("openssl", message, true);
    }
    if let Some(message) = verified_wrapper_message(cause) {
        return ("transport_wrapper", message.to_string(), false);
    }
    (
        "unrecognized",
        "unrecognized cause; message omitted".to_string(),
        true,
    )
}

const MAX_WRAPPER_MESSAGE_BYTES: usize = 160;

// Private transport wrapper types cannot be downcast outside hyper-util. Render
// at most 160 bytes for exact matching, then return a STATIC allowlisted literal,
// never the candidate text. A custom error impersonating a literal cannot expose
// its payload, and a new/dependency-changed message is visibly omitted.
fn verified_wrapper_message(error: &dyn StdError) -> Option<&'static str> {
    use std::{fmt, fmt::Write as _};
    struct Bounded(String);
    impl fmt::Write for Bounded {
        fn write_str(&mut self, text: &str) -> fmt::Result {
            if self.0.len().saturating_add(text.len()) > MAX_WRAPPER_MESSAGE_BYTES {
                return Err(fmt::Error);
            }
            self.0.push_str(text);
            Ok(())
        }
    }
    let mut candidate = Bounded(String::new());
    write!(&mut candidate, "{error}").ok()?;
    VERIFIED_WRAPPER_MESSAGES
        .iter()
        .copied()
        .find(|message| *message == candidate.0)
}

const VERIFIED_WRAPPER_MESSAGES: &[&str] = &[
    "dns error",
    "tcp connect error",
    "tcp open error",
    "tcp set_nonblocking error",
    "tcp bind interface error",
    "tcp bind local error",
    "error converting interface name to index",
    "invalid URL, scheme is not http",
    "invalid URL, scheme is missing",
    "invalid URL, host is missing",
    "tunnel error: missing destination host",
    "tunnel error: proxy authorization required",
    "tunnel error: proxy response headers too long",
    "tunnel error: unexpected end of file",
    "tunnel error: unsuccessful",
    "tunnel error: failed to create underlying connection",
    "tunnel error: io error establishing tunnel",
];

fn rustls_message(error: &rustls::Error) -> (String, bool) {
    use rustls::Error;
    let message = match error {
        Error::InvalidCertificate(certificate) => {
            return (
                format!(
                    "invalid peer certificate: {}",
                    certificate_message(certificate)
                ),
                true,
            )
        }
        // AlertDescription contains only a protocol enum/numeric value.
        Error::AlertReceived(alert) => return (format!("received TLS alert: {alert:?}"), false),
        Error::NoCertificatesPresented => "no certificates presented",
        Error::UnsupportedNameType => "unsupported certificate name type",
        Error::InappropriateMessage { .. } => "inappropriate TLS message",
        Error::InappropriateHandshakeMessage { .. } => "inappropriate TLS handshake message",
        Error::InvalidMessage(_) => "invalid TLS message",
        Error::PeerIncompatible(_) => "TLS peer incompatible",
        Error::PeerMisbehaved(_) => "TLS peer misbehaved",
        Error::DecryptError => "TLS decryption failed",
        Error::EncryptError => "TLS encryption failed",
        Error::InvalidCertRevocationList(_) => "invalid certificate revocation list",
        Error::FailedToGetCurrentTime => "failed to get current time",
        Error::FailedToGetRandomBytes => "failed to get random bytes",
        Error::HandshakeNotComplete => "TLS handshake not complete",
        Error::PeerSentOversizedRecord => "TLS peer sent oversized record",
        Error::NoApplicationProtocol => "no common TLS application protocol",
        Error::BadMaxFragmentSize => "invalid TLS maximum fragment size",
        Error::InconsistentKeys(_) => "TLS certificate/key mismatch",
        Error::General(_) => "TLS general error; details omitted",
        Error::Other(_) => "TLS backend error; details omitted",
        _ => "TLS error; details omitted",
    };
    (message.to_string(), true)
}

fn certificate_message(error: &rustls::CertificateError) -> &'static str {
    use rustls::CertificateError;
    match error {
        CertificateError::BadEncoding => "bad encoding",
        CertificateError::Expired | CertificateError::ExpiredContext { .. } => "expired",
        CertificateError::NotValidYet | CertificateError::NotValidYetContext { .. } => {
            "not valid yet"
        }
        CertificateError::Revoked => "revoked",
        CertificateError::UnhandledCriticalExtension => "unhandled critical extension",
        CertificateError::UnknownIssuer => "unknown issuer",
        CertificateError::UnknownRevocationStatus => "unknown revocation status",
        CertificateError::ExpiredRevocationList
        | CertificateError::ExpiredRevocationListContext { .. } => "expired revocation list",
        CertificateError::BadSignature => "bad signature",
        CertificateError::UnsupportedSignatureAlgorithmContext { .. }
        | CertificateError::UnsupportedSignatureAlgorithmForPublicKeyContext { .. } => {
            "unsupported signature algorithm"
        }
        CertificateError::NotValidForName | CertificateError::NotValidForNameContext { .. } => {
            "not valid for server name (names omitted)"
        }
        CertificateError::InvalidPurpose | CertificateError::InvalidPurposeContext { .. } => {
            "invalid certificate purpose"
        }
        CertificateError::InvalidOcspResponse => "invalid OCSP response",
        CertificateError::ApplicationVerificationFailure => "application verification failed",
        _ => "certificate validation failed; details omitted",
    }
}

#[cfg(target_os = "linux")]
fn safe_openssl_cause(cause: &(dyn StdError + 'static)) -> Option<String> {
    let errors: Vec<_> = if let Some(stack) = cause.downcast_ref::<::openssl::error::ErrorStack>() {
        stack.errors().iter().collect()
    } else {
        vec![cause.downcast_ref::<::openssl::error::Error>()?]
    };
    let safe_errors: Vec<_> = errors
        .iter()
        .take(16)
        .map(|error| {
            serde_json::json!({
                "code": error.code(),
                "library_code": error.library_code(),
                "reason_code": error.reason_code(),
                "library": error.library(),
                "reason": error.reason(),
            })
        })
        .collect();
    Some(
        serde_json::json!({
            "errors": safe_errors,
            "stack_truncated": errors.len() > 16,
            "attached_data_omitted": true,
        })
        .to_string(),
    )
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
        spawn_server_with_reset(close_first, false).await
    }

    async fn spawn_server_with_reset(
        close_first: usize,
        reset: bool,
    ) -> (String, Arc<AtomicUsize>) {
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
                    } else if reset {
                        stream.set_zero_linger().unwrap();
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

    #[tokio::test]
    async fn request_errors_after_transmission_are_not_retried() {
        let (url, seen) = spawn_server_with_reset(1, true).await;
        let factory_calls = AtomicUsize::new(0);
        let (response, retried) = send_request_with_retry(reqwest::Client::new().post(url), || {
            factory_calls.fetch_add(1, Ordering::SeqCst);
            Ok(reqwest::Client::new())
        })
        .await;
        assert!(!retried);
        assert_eq!(factory_calls.load(Ordering::SeqCst), 0);
        assert_eq!(seen.load(Ordering::SeqCst), 1);
        assert_eq!(
            response.unwrap_err().current_context(),
            &ApiClientError::RequestNotSent("request error".to_string())
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod transport_diagnostic_tests {
    use super::*;
    use std::fmt;
    use std::{
        io::Write,
        sync::{Arc, Mutex},
    };
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    const SECRET: &str = "SENTINEL_API_KEY_CARD_CERTIFICATE";

    #[derive(Debug)]
    struct CustomError(String);
    impl fmt::Display for CustomError {
        fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str(&self.0)
        }
    }
    impl StdError for CustomError {}

    #[derive(Clone, Default)]
    struct Capture(Arc<Mutex<Vec<u8>>>);
    impl Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .map_err(|_| std::io::Error::other("log capture lock poisoned"))?
                .extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    fn captured_sources(error: reqwest::Error) -> Vec<serde_json::Value> {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            let _ = map_reqwest_error(error);
        });
        let bytes = capture.0.lock().unwrap();
        let log = std::str::from_utf8(&bytes).unwrap();
        assert!(
            !log.contains(SECRET),
            "sensitive marker leaked into diagnostic"
        );
        assert!(!log.contains("http://") && !log.contains("https://"));
        let event: serde_json::Value = serde_json::from_str(log).unwrap();
        let sources: serde_json::Value = serde_json::from_str(
            event
                .get("fields")
                .unwrap()
                .get("error_sources")
                .unwrap()
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert!(sources
            .as_array()
            .is_some_and(|sources| !sources.is_empty()));
        sources.as_array().unwrap().clone()
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn openssl_reason_survives_without_attached_data() {
        let stack = ::openssl::pkey::PKey::private_key_from_pem(SECRET.as_bytes()).unwrap_err();
        let safe = safe_openssl_cause(&stack).unwrap();
        let value: serde_json::Value = serde_json::from_str(&safe).unwrap();
        let errors = value.get("errors").unwrap().as_array().unwrap();
        let error = errors.first().unwrap();
        assert!(error.get("code").unwrap().is_number());
        assert!(error.get("reason").unwrap().is_string());
        assert_eq!(value.get("attached_data_omitted").unwrap(), true);
        assert!(!safe.contains(SECRET));
    }

    #[tokio::test]
    async fn fresh_client_failure_does_not_log_report_attachments() {
        let capture = Capture::default();
        let writer = capture.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        let _guard = tracing::subscriber::set_default(subscriber);
        let endpoint = responding_socket(b"").await;
        let (_, retried) = send_request_with_retry(reqwest::Client::new().get(endpoint), || {
            Err(report!(ApiClientError::ClientConstructionFailed).attach_printable(SECRET))
        })
        .await;
        assert!(retried);
        let bytes = capture.0.lock().unwrap();
        let log = std::str::from_utf8(&bytes).unwrap();
        assert!(log.contains("Cannot build fresh retry client"));
        let expected = ApiClientError::ClientConstructionFailed.to_string();
        assert!(log.lines().any(|line| {
            let event: serde_json::Value = serde_json::from_str(line).unwrap();
            event
                .get("fields")
                .and_then(|fields| fields.get("client_error"))
                .and_then(serde_json::Value::as_str)
                == Some(expected.as_str())
        }));
        assert!(!log.contains(SECRET));
    }

    #[test]
    fn unknown_and_tls_details_are_omitted() {
        let (_, message, omitted) = safe_transport_cause(&CustomError(SECRET.to_string()));
        assert!(omitted && !message.contains(SECRET));
        let (_, message, omitted) =
            safe_transport_cause(&rustls::Error::General(SECRET.to_string()));
        assert!(omitted && !message.contains(SECRET));
        let error =
            rustls::Error::InvalidCertificate(rustls::CertificateError::NotValidForNameContext {
                expected: "private.example".try_into().unwrap(),
                presented: vec![SECRET.to_string()],
            });
        let (_, message, omitted) = safe_transport_cause(&error);
        assert!(omitted && message.contains("not valid for server name"));
        assert!(!message.contains(SECRET) && !message.contains("private.example"));
    }

    #[test]
    fn private_wrapper_allowlist_is_exact_and_bounded() {
        for message in VERIFIED_WRAPPER_MESSAGES {
            assert_eq!(
                verified_wrapper_message(&CustomError(message.to_string())),
                Some(*message)
            );
            assert_eq!(
                verified_wrapper_message(&CustomError(format!("{message}: {SECRET}"))),
                None
            );
        }
        assert_eq!(
            verified_wrapper_message(&CustomError(SECRET.repeat(1000))),
            None
        );
    }

    #[test]
    fn os_error_code_and_platform_message_survive() {
        let io = std::io::Error::from_raw_os_error(111);
        let (kind, message, omitted) = safe_transport_cause(&io);
        assert_eq!(kind, "io");
        assert_eq!(message, io.to_string());
        assert!(!omitted);
    }

    async fn responding_socket(response: &'static [u8]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = [0u8; 4096];
            let _ = socket.read(&mut buffer).await;
            let _ = socket.write_all(response).await;
        });
        url
    }

    #[tokio::test]
    async fn proxy_rejection_and_eof_keep_actual_wrapper_messages_in_logs() {
        for (response, expected) in [
            (
                b"HTTP/1.1 407 Proxy Authentication Required\r\nContent-Length: 0\r\n\r\n"
                    .as_slice(),
                "tunnel error: proxy authorization required",
            ),
            (
                b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n".as_slice(),
                "tunnel error: unsuccessful",
            ),
            (b"".as_slice(), "tunnel error: unexpected end of file"),
        ] {
            let proxy = responding_socket(response).await;
            let client = reqwest::Client::builder()
                .proxy(reqwest::Proxy::all(proxy).unwrap())
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap();
            let error = client
                .post(format!("https://private.example/?api_key={SECRET}"))
                .header("Authorization", format!("Bearer {SECRET}"))
                .body(SECRET)
                .send()
                .await
                .unwrap_err();
            let sources = captured_sources(error);
            assert!(sources.iter().any(|source| source["kind"] == "hyper_util"));
            assert!(
                sources.iter().any(|source| source["message"] == expected),
                "expected proxy cause {expected}"
            );
        }
    }

    #[tokio::test]
    async fn dns_wrapper_survives_and_custom_resolver_text_does_not() {
        #[derive(Debug)]
        struct Resolver;
        impl reqwest::dns::Resolve for Resolver {
            fn resolve(&self, _: reqwest::dns::Name) -> reqwest::dns::Resolving {
                Box::pin(async {
                    let error: Box<dyn StdError + Send + Sync> =
                        Box::new(std::io::Error::other(CustomError(SECRET.to_string())));
                    Err(error)
                })
            }
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .dns_resolver(Arc::new(Resolver))
            .build()
            .unwrap();
        let error = client
            .get(format!("http://private.example/?api_key={SECRET}"))
            .send()
            .await
            .unwrap_err();
        let sources = captured_sources(error);
        assert!(sources
            .iter()
            .any(|source| source["message"] == "dns error"));
        assert!(sources
            .iter()
            .any(|source| source["kind"] == "unrecognized" && source["details_omitted"] == true));
    }

    #[tokio::test]
    async fn native_and_rustls_handshake_failures_have_visible_backend() {
        for rustls in [false, true] {
            let endpoint = responding_socket(b"HTTP/1.1 200 OK\r\n\r\n").await;
            let builder = reqwest::Client::builder()
                .no_proxy()
                .timeout(std::time::Duration::from_secs(5));
            let client = if rustls {
                builder.use_rustls_tls()
            } else {
                builder // reqwest default-tls uses native TLS; rustls is selected explicitly above.
            }
            .build()
            .unwrap();
            let url = format!(
                "{}?api_key={SECRET}",
                endpoint.replacen("http://", "https://", 1)
            );
            let error = client.get(url).send().await.unwrap_err();
            let sources = captured_sources(error);
            let backend = if rustls { "rustls" } else { "native_tls" };
            assert!(
                sources.iter().any(|source| source["kind"] == backend),
                "missing {backend} source"
            );
        }
    }
}
