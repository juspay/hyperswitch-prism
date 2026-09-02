//! Déjà record/replay wiring for the gRPC server.
//!
//! Feature-gated and **inert until a boot hook is installed** (a later commit): with no
//! hook, `deja::process_runtime_mode()` is `Disabled`, every predicate here is false, and
//! the ingress layer is a pure passthrough. So even feature-on, the server behaves exactly
//! as feature-off until recording is deliberately switched on.

pub mod boot;
pub mod descriptors;
pub mod http_layer;
pub mod layer;
pub mod record_sink;
pub mod sampler;

use std::sync::{Arc, OnceLock};

use base64::Engine as _;

/// The process-wide déjà runtime hook, peeked once. `None` until boot installs one
/// (which keeps the ingress layer inert). Used to obtain the `&dyn DejaHook` /
/// `Arc<dyn DejaHook>` an event needs.
static HOOK: OnceLock<Option<Arc<deja::RuntimeHook>>> = OnceLock::new();

/// The installed runtime hook, if any.
pub fn hook() -> Option<&'static Arc<deja::RuntimeHook>> {
    HOOK.get_or_init(deja::global_runtime_hook_from_env)
        .as_ref()
}

/// Whether the process is recording or replaying.
///
/// The ingress boundary gates on this **boot-time process mode**, never the per-request
/// recording decision — the ingress is what *pushes* that decision, so gating on it would
/// be circular (nothing would ever record). See [`deja::process_runtime_mode`].
pub fn process_is_active() -> bool {
    !deja::process_runtime_mode().is_disabled()
}

/// Whether the process is in record mode (so a per-request sampling decision is pushed).
pub fn process_is_record_mode() -> bool {
    deja::process_runtime_mode().is_record()
}

/// Build the `grpc_incoming` event args: rpc path, authority, sorted metadata, and the
/// request message. The raw wire bytes (`raw_b64`, gRPC-framed exactly as read off the
/// transport) are ALWAYS recorded — they are what a replay driver re-sends, descriptors
/// or not — with the proto3-JSON `decoded` alongside when the schema is known.
/// Metadata is sorted by (name, value) because `HeaderMap` iteration order is not stable
/// and the args are used for identity.
pub fn grpc_incoming_args(
    rpc: &str,
    authority: Option<&str>,
    headers: &http::HeaderMap,
    request_bytes: &[u8],
    decoded: Option<serde_json::Value>,
) -> serde_json::Value {
    let mut metadata: Vec<(String, String)> = headers
        .iter()
        .filter_map(|(name, value)| {
            value
                .to_str()
                .ok()
                .map(|value| (name.as_str().to_owned(), value.to_owned()))
        })
        .collect();
    metadata.sort();

    let raw_b64 = base64::engine::general_purpose::STANDARD.encode(request_bytes);
    let request = match decoded {
        Some(json) => serde_json::json!({ "raw_b64": raw_b64, "decoded": json }),
        None => serde_json::json!({ "raw_b64": raw_b64, "undecoded": true }),
    };

    serde_json::json!({
        "rpc": rpc,
        "authority": authority,
        "metadata": metadata,
        "request": request,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]
mod tests {
    #[test]
    fn inert_without_a_hook() {
        // With no boot hook installed (the default in tests, and in production until the
        // install commit lands), the process mode is Disabled — so the ingress layer's
        // first check takes the pure-passthrough arm and nothing is captured.
        assert!(!super::process_is_active());
        assert!(!super::process_is_record_mode());
    }

    #[test]
    fn grpc_incoming_args_sorts_metadata_and_marks_undecoded() {
        let mut headers = http::HeaderMap::new();
        headers.insert("x-request-id", "req-1".parse().unwrap());
        headers.insert("x-connector", "adyen".parse().unwrap());
        let args = super::grpc_incoming_args(
            "/ucs.v2.PaymentService/Authorize",
            Some("localhost"),
            &headers,
            &[1, 2, 3],
            None,
        );
        assert_eq!(args["rpc"], "/ucs.v2.PaymentService/Authorize");
        // Sorted by name: x-connector before x-request-id.
        assert_eq!(args["metadata"][0][0], "x-connector");
        assert_eq!(args["metadata"][1][0], "x-request-id");
        assert_eq!(args["request"]["undecoded"], true);
        // The raw wire bytes ride EVERY recording (decoded or not) — they are
        // what the replay driver re-sends.
        assert_eq!(args["request"]["raw_b64"], "AQID"); // b64 of [1, 2, 3]
    }

    #[test]
    fn grpc_incoming_args_keeps_raw_bytes_alongside_decoded() {
        let args = super::grpc_incoming_args(
            "/ucs.v2.PaymentService/Authorize",
            None,
            &http::HeaderMap::new(),
            &[1, 2, 3],
            Some(serde_json::json!({"amount": 1})),
        );
        assert_eq!(args["request"]["raw_b64"], "AQID");
        assert_eq!(args["request"]["decoded"]["amount"], 1);
        assert!(args["request"].get("undecoded").is_none());
    }
}
