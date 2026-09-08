//! Per-request recording sampler: the seam AND the Superposition-backed policy.
//!
//! The déjà library does not ship the trait (it is the integrator's policy), so it is
//! declared here, together with [`SuperpositionRecordingSampler`] — the concrete policy a
//! record-mode process installs. The decision (`deja_record`, default FALSE) is evaluated
//! in-process against the boot-parsed `config/superposition.toml`, dimensioned on
//! `environment` × `rpc_method` × `rpc_service` (the service class derived from the
//! path), memoized per rpc (the snapshot is frozen at boot, so a memo can never go
//! stale), and fail-closed: every failure path — no snapshot, key missing, eval error —
//! resolves to `!fail_closed`, which by default records nothing. Alongside the boolean,
//! `deja_record_percent` (0-100, default 0) samples a deterministic fraction of the
//! remainder: the request id hashes to a stable bucket, so "record 8% of the payment
//! class" is one override — hyperswitch's experiment posture, expressed in the file.

use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use common_utils::{consts::Env, superposition_config::SuperpositionConfig};

/// Facts the sampler decides on. Deliberately minimal and cheap to build.
pub struct RequestRecordingFacts {
    pub request_id: String,
    pub rpc: String,
}

/// Decides whether a given request should be recorded. Consulted once per request in
/// record mode, before the handler runs.
pub trait RequestRecordingSampler: Send + Sync {
    fn should_record(
        &self,
        facts: RequestRecordingFacts,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + '_>>;
}

/// The service class of an rpc path, derived structurally — `/types.PaymentService/
/// Authorize` → `payment` — so per-class overrides need no maintained prefix map
/// (hyperswitch needs a server-side cohort for this because ITS derivation runs
/// remotely; ours is this function). Malformed paths class as `unknown`.
fn rpc_service(rpc: &str) -> String {
    let service = rpc
        .trim_start_matches('/')
        .split('/')
        .next()
        .and_then(|package| package.rsplit('.').next())
        .unwrap_or("")
        .trim_end_matches("Service");
    if service.is_empty() {
        "unknown".to_owned()
    } else {
        service.to_lowercase()
    }
}

/// What the policy resolved to for one rpc: the wholesale boolean plus the
/// percentage gate. The POLICY is per-rpc and memoizable; the percentage
/// DECISION is per-request (it hashes the request id) and never cached.
#[derive(Clone, Copy)]
struct ResolvedPolicy {
    record: bool,
    percent: u8,
}

/// A request id's stable bucket in [0, 100). FNV-1a, so the same request id
/// lands in the same bucket on every pod and every boot — percentage sampling
/// is deterministic per request, exactly like hyperswitch's targeting key.
fn request_bucket(request_id: &str) -> u8 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in request_id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    u8::try_from(hash % 100).unwrap_or(0)
}

/// The Superposition-backed recording policy (see the module doc). Built once at
/// install time, exactly when the process is in record mode.
pub struct SuperpositionRecordingSampler {
    /// `None` = record mode booted with no superposition snapshot: the no-source
    /// state, where every decision is `!fail_closed` (the install site logged it).
    superposition: Option<Arc<SuperpositionConfig>>,
    environment: &'static str,
    record_key: String,
    /// `<record_key>_percent`: the percentage gate, composing with a custom key.
    percent_key: String,
    fail_closed: bool,
    /// rpc path → resolved POLICY (not decision — percentages decide per request).
    /// Sound because the snapshot is frozen at boot; failure policies memoize
    /// too, which also bounds their warn to once per rpc.
    memo: std::sync::RwLock<HashMap<String, ResolvedPolicy>>,
}

impl SuperpositionRecordingSampler {
    pub fn from_config(config: &ucs_env::configs::Config) -> Self {
        let sampler = &config.deja.sampler;
        if config.superposition_config.is_none() {
            tracing::error!(
                fail_closed = sampler.fail_closed,
                "deja record mode with no sampling source (superposition.toml did not load); \
                 every request resolves to the configured failure default"
            );
        }
        Self::assemble(
            config.superposition_config.clone(),
            match config.common.environment {
                Env::Development => "development",
                Env::Production => "production",
                Env::Sandbox => "sandbox",
            },
            sampler,
        )
    }

    fn assemble(
        superposition: Option<Arc<SuperpositionConfig>>,
        environment: &'static str,
        sampler: &ucs_env::deja_config::SamplerConfig,
    ) -> Self {
        Self {
            superposition,
            environment,
            percent_key: format!("{}_percent", sampler.record_key),
            record_key: sampler.record_key.clone(),
            fail_closed: sampler.fail_closed,
            memo: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn decide(&self, facts: &RequestRecordingFacts) -> bool {
        let policy = self.policy_for(&facts.rpc);
        // The boolean records wholesale; the percentage samples the remainder.
        policy.record
            || (policy.percent > 0 && request_bucket(&facts.request_id) < policy.percent)
    }

    fn policy_for(&self, rpc: &str) -> ResolvedPolicy {
        if let Some(hit) = self.memo.read().ok().and_then(|memo| memo.get(rpc).copied()) {
            return hit;
        }

        let failure_default = !self.fail_closed;
        let failure_policy = ResolvedPolicy {
            record: failure_default,
            percent: 0,
        };
        let policy = match &self.superposition {
            None => failure_policy, // no-source: logged once at install
            Some(superposition) => match superposition.resolve_with(&[
                ("environment", self.environment),
                ("rpc_method", rpc),
                ("rpc_service", &rpc_service(rpc)),
            ]) {
                Ok(resolved) => {
                    let record = match resolved.get(&self.record_key).and_then(|v| v.as_bool()) {
                        Some(decision) => decision,
                        None => {
                            tracing::warn!(
                                record_key = %self.record_key,
                                rpc = %rpc,
                                failure_default,
                                "deja sampler key missing or non-boolean in superposition \
                                 snapshot; using configured failure default"
                            );
                            failure_default
                        }
                    };
                    // An absent percent key simply means no percentage sampling;
                    // a present non-integer one is a config mistake worth a warn.
                    let percent = match resolved.get(&self.percent_key) {
                        None => 0,
                        Some(value) => value.as_u64().map_or_else(
                            || {
                                tracing::warn!(
                                    percent_key = %self.percent_key,
                                    rpc = %rpc,
                                    "deja sampler percent is not a non-negative integer; \
                                     treating as 0"
                                );
                                0
                            },
                            |n| u8::try_from(n.min(100)).unwrap_or(100),
                        ),
                    };
                    ResolvedPolicy { record, percent }
                }
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        rpc = %rpc,
                        failure_default,
                        "deja sampler evaluation failed; using configured failure default"
                    );
                    failure_policy
                }
            },
        };
        if let Ok(mut memo) = self.memo.write() {
            memo.insert(rpc.to_owned(), policy);
        }
        policy
    }
}

impl RequestRecordingSampler for SuperpositionRecordingSampler {
    fn should_record(
        &self,
        facts: RequestRecordingFacts,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        // Evaluation is in-process against a boot-frozen snapshot — synchronous,
        // nothing to time out (the RFC's documented deviation from hyperswitch's
        // networked client).
        let decision = self.decide(&facts);
        Box::pin(std::future::ready(decision))
    }
}

/// Clears the per-correlation recording decision on drop (covers `?`, panic, cancel).
/// Shared by the gRPC and HTTP ingress layers.
pub(crate) struct RecordingDecisionGuard(pub(crate) String);

impl Drop for RecordingDecisionGuard {
    fn drop(&mut self) {
        deja::clear_recording_decision(&self.0);
    }
}

#[cfg(test)]
mod tests {
    // Fixture assertions may unwrap/index freely — a panic IS the test failing.
    #![allow(clippy::expect_used, clippy::unwrap_used, clippy::indexing_slicing)]

    use super::*;

    const AUTHORIZE: &str = "/types.PaymentService/Authorize";

    /// A minimal snapshot in the exact shape of config/superposition.toml:
    /// default SKIP, development sampled in wholesale, production sampled in
    /// for one rpc only.
    const POLICY: &str = r#"
[default-configs]
deja_record = { value = false, schema = { type = "boolean" } }

[dimensions]
environment = { position = 1, schema = { type = "string" } }
rpc_method = { position = 2, schema = { type = "string" } }

[[overrides]]
_context_ = { environment = "development" }
deja_record = true

[[overrides]]
_context_ = { environment = "production", rpc_method = "/types.PaymentService/Authorize" }
deja_record = true
"#;

    fn snapshot(content: &str) -> Arc<SuperpositionConfig> {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "deja-sampler-test-{}-{}.toml",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::write(&path, content).unwrap();
        let config = SuperpositionConfig::from_file(path.to_str().unwrap()).unwrap();
        std::fs::remove_file(&path).ok();
        Arc::new(config)
    }

    fn sampler_cfg(fail_closed: bool) -> ucs_env::deja_config::SamplerConfig {
        ucs_env::deja_config::SamplerConfig {
            record_key: "deja_record".to_string(),
            fail_closed,
        }
    }

    fn facts(rpc: &str) -> RequestRecordingFacts {
        RequestRecordingFacts {
            request_id: "req-1".to_string(),
            rpc: rpc.to_string(),
        }
    }

    /// Production defaults dark: no matching override => skip; the targeted
    /// rpc override samples exactly that rpc in.
    #[test]
    fn production_defaults_dark_with_targeted_sample_in() {
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(POLICY)),
            "production",
            &sampler_cfg(true),
        );
        assert!(sampler.decide(&facts(AUTHORIZE)), "targeted override samples in");
        assert!(
            !sampler.decide(&facts("/types.PaymentService/Refund")),
            "everything else inherits the dark default"
        );
    }

    /// The development override keeps the local rig recording wholesale.
    #[test]
    fn development_records_everything() {
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(POLICY)),
            "development",
            &sampler_cfg(true),
        );
        assert!(sampler.decide(&facts(AUTHORIZE)));
        assert!(sampler.decide(&facts("/types.PaymentService/Refund")));
    }

    /// A snapshot without the record key resolves to the configured failure
    /// default, both ways.
    #[test]
    fn missing_key_uses_failure_default_both_ways() {
        const KEYLESS: &str = r#"
[default-configs]
unrelated = { value = true, schema = { type = "boolean" } }

[dimensions]
environment = { position = 1, schema = { type = "string" } }

[[overrides]]
_context_ = { environment = "development" }
unrelated = false
"#;
        let closed = SuperpositionRecordingSampler::assemble(
            Some(snapshot(KEYLESS)),
            "production",
            &sampler_cfg(true),
        );
        assert!(!closed.decide(&facts(AUTHORIZE)), "fail-closed skips");
        let open = SuperpositionRecordingSampler::assemble(
            Some(snapshot(KEYLESS)),
            "production",
            &sampler_cfg(false),
        );
        assert!(open.decide(&facts(AUTHORIZE)), "fail-open records");
    }

    /// Record mode with no snapshot at all: every decision is the failure
    /// default (the install site logged the condition).
    #[test]
    fn no_source_uses_failure_default() {
        let sampler =
            SuperpositionRecordingSampler::assemble(None, "production", &sampler_cfg(true));
        assert!(!sampler.decide(&facts(AUTHORIZE)));
        let open =
            SuperpositionRecordingSampler::assemble(None, "production", &sampler_cfg(false));
        assert!(open.decide(&facts(AUTHORIZE)));
    }

    /// The service class derives structurally from the path — no map to maintain.
    #[test]
    fn rpc_service_derives_from_the_path() {
        assert_eq!(rpc_service("/types.PaymentService/Authorize"), "payment");
        assert_eq!(rpc_service("/types.RefundService/Refund"), "refund");
        assert_eq!(rpc_service("/grpc.health.v1.Health/Check"), "health");
        assert_eq!(rpc_service(""), "unknown");
    }

    /// A class-targeted override samples in every rpc of that service and none
    /// of any other — the file-side analogue of hyperswitch's cohort context.
    #[test]
    fn class_override_targets_the_whole_service() {
        const BY_CLASS: &str = r#"
[default-configs]
deja_record = { value = false, schema = { type = "boolean" } }

[dimensions]
environment = { position = 1, schema = { type = "string" } }
rpc_service = { position = 2, schema = { type = "string" } }

[[overrides]]
_context_ = { environment = "production", rpc_service = "payment" }
deja_record = true
"#;
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(BY_CLASS)),
            "production",
            &sampler_cfg(true),
        );
        assert!(sampler.decide(&facts("/types.PaymentService/Authorize")));
        assert!(sampler.decide(&facts("/types.PaymentService/Capture")));
        assert!(!sampler.decide(&facts("/types.RefundService/Refund")));
    }

    /// Percentage sampling: deterministic per request id, boolean wins
    /// wholesale, 0/absent never gates in, 100 always does.
    #[test]
    fn percent_gates_deterministically_by_request_id() {
        const PCT: &str = r#"
[default-configs]
deja_record = { value = false, schema = { type = "boolean" } }
deja_record_percent = { value = 0, schema = { type = "integer" } }

[dimensions]
environment = { position = 1, schema = { type = "string" } }

[[overrides]]
_context_ = { environment = "production" }
deja_record_percent = 50
"#;
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(PCT)),
            "production",
            &sampler_cfg(true),
        );
        let with_id = |id: &str| RequestRecordingFacts {
            request_id: id.to_owned(),
            rpc: AUTHORIZE.to_owned(),
        };
        let ids: Vec<String> = (0..999).map(|i| format!("req-{i}")).collect();
        let low = ids.iter().find(|id| request_bucket(id) < 50).unwrap();
        let high = ids.iter().find(|id| request_bucket(id) >= 50).unwrap();
        assert!(sampler.decide(&with_id(low)), "bucket below the gate records");
        assert!(!sampler.decide(&with_id(high)), "bucket at/above the gate skips");
        assert!(
            sampler.decide(&with_id(low)) && !sampler.decide(&with_id(high)),
            "same request id, same answer — the gate is deterministic"
        );
    }

    #[test]
    fn percent_boundaries_and_boolean_precedence() {
        const EDGES: &str = r#"
[default-configs]
deja_record = { value = false, schema = { type = "boolean" } }
deja_record_percent = { value = 0, schema = { type = "integer" } }

[dimensions]
environment = { position = 1, schema = { type = "string" } }
rpc_method = { position = 2, schema = { type = "string" } }

[[overrides]]
_context_ = { environment = "production", rpc_method = "/types.PaymentService/Authorize" }
deja_record_percent = 100

[[overrides]]
_context_ = { environment = "development" }
deja_record = true
"#;
        let production = SuperpositionRecordingSampler::assemble(
            Some(snapshot(EDGES)),
            "production",
            &sampler_cfg(true),
        );
        assert!(production.decide(&facts(AUTHORIZE)), "percent 100 always records");
        assert!(
            !production.decide(&facts("/types.RefundService/Refund")),
            "percent 0 (default) never gates in"
        );
        let development = SuperpositionRecordingSampler::assemble(
            Some(snapshot(EDGES)),
            "development",
            &sampler_cfg(true),
        );
        assert!(
            development.decide(&facts(AUTHORIZE)),
            "the wholesale boolean records regardless of percent"
        );
    }

    /// Decisions memoize per rpc — sound because the snapshot is boot-frozen.
    #[test]
    fn memoizes_per_rpc() {
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(POLICY)),
            "production",
            &sampler_cfg(true),
        );
        sampler.decide(&facts(AUTHORIZE));
        let hit = sampler.memo.read().unwrap().get(AUTHORIZE).copied();
        assert!(
            matches!(hit, Some(ResolvedPolicy { record: true, percent: 0 })),
            "first consult lands the resolved policy in the memo"
        );
    }


}
