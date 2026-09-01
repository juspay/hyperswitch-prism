//! Deterministic, mask-safe span fields for the `ucs::connector_call` span —
//! the connector call's essential facts (url, headers, body, status,
//! integrity outcome), captured for the déjà execution graph and compared
//! record-vs-replay by the span-shape check.
//!
//! HARNESS INSTRUMENTATION, not behavior: nothing here changes what prism
//! sends — only what the instrument records about it. Three rules govern
//! every value:
//!
//!   - DETERMINISTIC BY CONSTRUCTION. `Headers` is a HashSet and request
//!     bodies serialize maps in per-process-random order; every recorded
//!     value sorts (BTree) before joining or hashing, so identical behavior
//!     records identical bytes across processes. The comparator never needs
//!     to normalize these.
//!   - MASK-SAFE. Span fields flow to logs and to the tape. Masked header
//!     values are digested, never inlined; URL query VALUES are dropped
//!     entirely (multisafepay carries `api_key` there — the only connector
//!     that puts credentials in a URL); bodies are digested over their
//!     MASKED serialization, so no raw PAN bytes are ever hashed here.
//!   - COMPARABLE, NOT READABLE-BACK. A digest says "same or different",
//!     which is the harness's question; the verbatim payloads already live
//!     on the boundary event where replay needs them.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use common_utils::crypto::GenerateDigest as _;
use common_utils::request::{Request, RequestContent};
use hyperswitch_masking::{ErasedMaskSerialize, Maskable, PeekInterface};

/// Hex sha256, same shape as `deja_codec::injector_args`' digest.
fn sha256_hex(bytes: &[u8]) -> String {
    common_utils::crypto::Sha256
        .generate_digest(bytes)
        .map(|digest| {
            digest.iter().fold(String::with_capacity(64), |mut hex, b| {
                let _ = write!(hex, "{b:02x}");
                hex
            })
        })
        .unwrap_or_default()
}

/// Serialize a JSON value with every object's keys sorted, recursively —
/// a canonical byte form that is independent of map iteration order and of
/// serde_json's map-backing feature flags.
fn canonical_json_bytes(value: &serde_json::Value) -> Vec<u8> {
    fn canonicalize(value: &serde_json::Value, out: &mut Vec<u8>) {
        match value {
            serde_json::Value::Object(map) => {
                let sorted: BTreeMap<&String, &serde_json::Value> = map.iter().collect();
                out.push(b'{');
                for (i, (k, v)) in sorted.into_iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    out.extend_from_slice(
                        serde_json::to_string(k).unwrap_or_default().as_bytes(),
                    );
                    out.push(b':');
                    canonicalize(v, out);
                }
                out.push(b'}');
            }
            serde_json::Value::Array(items) => {
                out.push(b'[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(b',');
                    }
                    canonicalize(item, out);
                }
                out.push(b']');
            }
            leaf => {
                out.extend_from_slice(serde_json::to_string(leaf).unwrap_or_default().as_bytes())
            }
        }
    }
    let mut out = Vec::new();
    canonicalize(value, &mut out);
    out
}

/// `(origin, path, sorted query KEY names)` — the URL split that never
/// carries a credential. Unparseable URLs record verbatim under origin (a
/// broken URL is itself a fact worth comparing).
pub fn split_url(raw: &str) -> (String, String, String) {
    match url::Url::parse(raw) {
        Ok(url) => {
            let mut keys: Vec<String> = url.query_pairs().map(|(k, _)| k.into_owned()).collect();
            keys.sort();
            keys.dedup();
            (url.origin().ascii_serialization(), url.path().to_owned(), keys.join(","))
        }
        Err(_) => (raw.to_owned(), String::new(), String::new()),
    }
}

/// Sorted header names, comma-joined.
pub fn header_names(headers: &common_utils::request::Headers) -> String {
    let mut names: Vec<&str> = headers.iter().map(|(name, _)| name.as_str()).collect();
    names.sort_unstable();
    names.join(",")
}

/// One digest over the whole header map, name→value with masked values
/// PRE-digested — so a rotated credential changes the digest (comparable)
/// while the tape's span field never carries the secret itself.
pub fn headers_digest(headers: &common_utils::request::Headers) -> String {
    let folded: BTreeMap<&str, String> = headers
        .iter()
        .map(|(name, value)| {
            let v = match value {
                Maskable::Normal(v) => v.clone(),
                Maskable::Masked(secret) => sha256_hex(secret.peek().as_bytes()),
            };
            (name.as_str(), v)
        })
        .collect();
    let mut bytes = Vec::new();
    for (name, value) in &folded {
        bytes.extend_from_slice(name.as_bytes());
        bytes.push(b'=');
        bytes.extend_from_slice(value.as_bytes());
        bytes.push(b'\n');
    }
    sha256_hex(&bytes)
}

/// Digest of the MASKED serialization in canonical (key-sorted) form — masked
/// first so no raw secret bytes are ever hashed, canonical so map order can't
/// change the digest. Plain `Serialize` on these boxes is the RAW wire form
/// (`get_inner_value` builds the actual request from it) — never use it here.
fn digest_masked(inner: &(dyn ErasedMaskSerialize + Send)) -> String {
    let masked = inner.masked_serialize().unwrap_or(serde_json::Value::Null);
    sha256_hex(&canonical_json_bytes(&masked))
}

/// `(kind, digest)` for the request body. Structured bodies digest their
/// masked canonical form; FormData digests its serialized part list; RawBytes
/// digests the bytes themselves (the only fingerprint an opaque body has).
pub fn body_facts(body: &Option<RequestContent>) -> (&'static str, String) {
    match body {
        None => ("none", String::new()),
        Some(RequestContent::Json(inner)) => ("json", digest_masked(inner.as_ref())),
        Some(RequestContent::FormUrlEncoded(inner)) => {
            ("form-urlencoded", digest_masked(inner.as_ref()))
        }
        Some(RequestContent::Xml(inner)) => ("xml", digest_masked(inner.as_ref())),
        Some(RequestContent::FormData(multipart)) => {
            let value = serde_json::to_value(multipart).unwrap_or(serde_json::Value::Null);
            ("form-data", sha256_hex(&canonical_json_bytes(&value)))
        }
        Some(RequestContent::RawBytes(bytes)) => ("raw-bytes", sha256_hex(bytes)),
    }
}

/// The `ucs::connector_call` span: one scored node carrying the call's
/// essential facts. Declared-`Empty` fields recorded as values materialize;
/// the graph layer's `on_record` folds deferred records into the node, which
/// is emitted at span close (drop).
pub struct ConnectorCallSpan(tracing::Span);

impl ConnectorCallSpan {
    pub fn open() -> Self {
        Self(tracing::info_span!(
            "ucs::connector_call",
            url.origin = tracing::field::Empty,
            url.path = tracing::field::Empty,
            url.query_keys = tracing::field::Empty,
            method = tracing::field::Empty,
            headers.names = tracing::field::Empty,
            headers.digest = tracing::field::Empty,
            body.kind = tracing::field::Empty,
            body.digest = tracing::field::Empty,
            status_code = tracing::field::Empty,
            integrity.result = tracing::field::Empty,
        ))
    }

    /// Record the request-side facts (HTTP arm, after `build_request_v2`).
    pub fn record_request(&self, request: &Request) {
        let (origin, path, query_keys) = split_url(&request.url);
        self.0.record("url.origin", origin.as_str());
        self.0.record("url.path", path.as_str());
        self.0.record("url.query_keys", query_keys.as_str());
        self.0
            .record("method", tracing::field::display(request.method));
        self.0
            .record("headers.names", header_names(&request.headers).as_str());
        self.0
            .record("headers.digest", headers_digest(&request.headers).as_str());
        let (kind, digest) = body_facts(&request.body);
        self.0.record("body.kind", kind);
        self.0.record("body.digest", digest.as_str());
    }

    /// Kafka arm: the topic stands where the origin would.
    pub fn record_topic(&self, topic: &str) {
        self.0.record("url.origin", topic);
        self.0.record("method", "kafka");
    }

    pub fn record_status(&self, status_code: Option<i32>) {
        if let Some(code) = status_code {
            self.0.record("status_code", code);
        }
    }

    /// `ok` covers both a passing comparison and a connector that populated
    /// no integrity object — the `CheckIntegrity` bound cannot tell them
    /// apart without a new trait surface, and inventing one here would be a
    /// core change. A failure names the mismatched fields.
    pub fn record_integrity(&self, result: Result<(), &str>) {
        match result {
            Ok(()) => self.0.record("integrity.result", "ok"),
            Err(field_names) => self
                .0
                .record("integrity.result", format!("failed:{field_names}").as_str()),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The whole point: identical content must digest identically no matter
    /// what order a per-process map hands it over in.
    #[test]
    fn canonical_digest_is_stable_across_key_order() {
        let a: serde_json::Value =
            serde_json::from_str(r#"{"b":1,"a":{"y":2,"x":[{"n":1,"m":2}]}}"#).unwrap();
        let b: serde_json::Value =
            serde_json::from_str(r#"{"a":{"x":[{"m":2,"n":1}],"y":2},"b":1}"#).unwrap();
        assert_eq!(canonical_json_bytes(&a), canonical_json_bytes(&b));
        // …and different content must not collide.
        let c: serde_json::Value = serde_json::from_str(r#"{"b":2,"a":{"y":2,"x":[]}}"#).unwrap();
        assert_ne!(canonical_json_bytes(&a), canonical_json_bytes(&c));
    }

    /// multisafepay puts `api_key` in the query string — the split must keep
    /// the key NAME (shape is behavior) and drop the value (a credential).
    #[test]
    fn url_split_keeps_query_keys_and_drops_values() {
        let (origin, path, keys) =
            split_url("https://testapi.multisafepay.com/v1/json/orders?api_key=SECRET&b=2");
        assert_eq!(origin, "https://testapi.multisafepay.com");
        assert_eq!(path, "/v1/json/orders");
        assert_eq!(keys, "api_key,b");
    }

    /// A masked header value must never appear in any recorded string, but a
    /// CHANGED masked value must change the digest — comparable, not visible.
    #[test]
    fn masked_header_values_never_surface_but_still_compare() {
        use hyperswitch_masking::Secret;
        let mk = |v: &str| -> common_utils::request::Headers {
            [
                ("Authorization".to_owned(), Maskable::Masked(Secret::new(v.to_owned()))),
                ("Content-Type".to_owned(), Maskable::Normal("application/json".to_owned())),
            ]
            .into_iter()
            .collect()
        };
        let a = mk("sk_test_topsecret");
        let names = header_names(&a);
        let digest = headers_digest(&a);
        assert!(!names.contains("topsecret") && !digest.contains("topsecret"));
        assert_eq!(names, "Authorization,Content-Type");
        assert_ne!(digest, headers_digest(&mk("sk_test_rotated")));
        assert_eq!(digest, headers_digest(&mk("sk_test_topsecret")));
    }
}
