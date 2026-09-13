//! Per-request recording sampler: the seam AND the Superposition-backed policy.
//!
//! The déjà library does not ship the trait (it is the integrator's policy), so it is
//! declared here, together with [`SuperpositionRecordingSampler`] — the concrete policy a
//! record-mode process installs. The decision (`deja_record`, default FALSE) is evaluated
//! in-process against Superposition's local provider over `config/superposition.toml`,
//! dimensioned on `environment` × `rpc_method` × `rpc_service` (the service class derived
//! from the path), and fail-closed: every failure path — no snapshot, key missing, eval
//! error — resolves to `!fail_closed`, which by default records nothing. Alongside the
//! boolean, `deja_record_percent` (0-100, default 0) samples a deterministic fraction of
//! the remainder: the request id hashes to a stable bucket, so "record 8% of the payment
//! class" is one override — hyperswitch's experiment posture, expressed in the file.
//!
//! The policy is resolved PER REQUEST, never memoized: the provider refreshes its
//! snapshot (a poll of the remote workspace, or a watch on the baked file), so a policy
//! change reaches the next request. A memo keyed on rpc would silently serve the
//! pre-change policy forever — the provider's own cache is the memo, and it is the one
//! that knows when the source moved.
//!
//! The request id rides along as the experiment TARGETING KEY: on a remote source,
//! Superposition experiments (`deja_record` CONTROL/TEST variants, ramped from the
//! dashboard) bucket on it — hyperswitch's exact sampler posture. Without it no
//! experiment ever applies, silently. Precedence: a `true` from an override or an
//! experiment variant records wholesale; otherwise `deja_record_percent` samples the
//! remainder; any failure — no source, missing key, eval error, timeout — resolves to
//! `!fail_closed`.

use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc, time::Duration};

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

/// What the policy resolved to for one request: the wholesale boolean plus the
/// percentage gate. The percentage DECISION hashes the request id.
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
    /// Per-lookup budget; elapsed ⇒ failure default (hyperswitch parity).
    timeout_ms: u64,
    /// rpcs whose policy failure has already been warned about. Bounds each
    /// failure warn to once per rpc without caching the policy itself — a
    /// degraded snapshot must not flood the log on every request.
    warned: std::sync::Mutex<HashSet<String>>,
}

impl SuperpositionRecordingSampler {
    pub fn from_config(config: &ucs_env::configs::Config) -> Self {
        let sampler = &config.deja.sampler;
        match &config.superposition_config {
            None => tracing::error!(
                fail_closed = sampler.fail_closed,
                "deja record mode with no sampling source (superposition.toml did not load); \
                 every request resolves to the configured failure default"
            ),
            Some(source) => tracing::info!(
                source = %source.source(),
                experiments_supported = source.experiments_supported(),
                record_key = %sampler.record_key,
                fail_closed = sampler.fail_closed,
                timeout_ms = sampler.timeout_ms,
                "deja recording sampler installed"
            ),
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
            timeout_ms: sampler.timeout_ms,
            warned: std::sync::Mutex::new(HashSet::new()),
        }
    }

    async fn decide(&self, facts: &RequestRecordingFacts) -> bool {
        let policy = self.policy_for(&facts.rpc, &facts.request_id).await;
        // The boolean records wholesale; the percentage samples the remainder.
        policy.record || (policy.percent > 0 && request_bucket(&facts.request_id) < policy.percent)
    }

    fn count(outcome: &str) {
        external_services::shared_metrics::SUPERPOSITION_RESOLVE_TOTAL
            .with_label_values(&["sampler", outcome])
            .inc();
        // Mirror to the OTLP-exported instrument, like every other metric here.
        #[cfg(feature = "otel")]
        external_services::otel_metrics::record_superposition_resolution("sampler", outcome);
    }

    /// True the first time an rpc reports a given failure, false after — so a
    /// persistent misconfiguration is logged once per rpc, not once per request.
    fn first_warn(&self, rpc: &str, what: &str) -> bool {
        self.warned
            .lock()
            .map(|mut warned| warned.insert(format!("{what}:{rpc}")))
            .unwrap_or(true)
    }

    async fn policy_for(&self, rpc: &str, request_id: &str) -> ResolvedPolicy {
        let failure_default = !self.fail_closed;
        let failure_policy = ResolvedPolicy {
            record: failure_default,
            percent: 0,
        };
        let Some(superposition) = &self.superposition else {
            Self::count("no_source");
            return failure_policy; // no-source: logged once at install
        };
        // The request id is the experiment targeting key (see the module doc); the
        // timeout only ever trips on a stalled provider — evaluation is in-process.
        let service = rpc_service(rpc);
        let dimensions = [
            ("environment", self.environment),
            ("rpc_method", rpc),
            ("rpc_service", service.as_str()),
        ];
        let lookup = superposition.resolve_with(&dimensions, Some(request_id));
        let resolved =
            match tokio::time::timeout(Duration::from_millis(self.timeout_ms.max(1)), lookup).await
            {
                Ok(resolved) => resolved,
                Err(_elapsed) => {
                    Self::count("timeout");
                    if self.first_warn(rpc, "timeout") {
                        tracing::warn!(
                            timeout_ms = self.timeout_ms,
                            rpc = %rpc,
                            failure_default,
                            "deja sampler policy lookup timed out; using configured failure default"
                        );
                    }
                    return failure_policy;
                }
            };
        match resolved {
            Ok(resolved) => {
                let record = match resolved.get(&self.record_key).and_then(|v| v.as_bool()) {
                    Some(decision) => {
                        Self::count("hit");
                        decision
                    }
                    None => {
                        Self::count("key_missing");
                        if self.first_warn(rpc, "key") {
                            tracing::warn!(
                                record_key = %self.record_key,
                                rpc = %rpc,
                                failure_default,
                                "deja sampler key missing or non-boolean in superposition \
                                 snapshot; using configured failure default"
                            );
                        }
                        failure_default
                    }
                };
                // An absent percent key simply means no percentage sampling;
                // a present non-integer one is a config mistake worth a warn.
                let percent = match resolved.get(&self.percent_key) {
                    None => 0,
                    Some(value) => value.as_u64().map_or_else(
                        || {
                            if self.first_warn(rpc, "percent") {
                                tracing::warn!(
                                    percent_key = %self.percent_key,
                                    rpc = %rpc,
                                    "deja sampler percent is not a non-negative integer; \
                                     treating as 0"
                                );
                            }
                            0
                        },
                        |n| u8::try_from(n.min(100)).unwrap_or(100),
                    ),
                };
                ResolvedPolicy { record, percent }
            }
            Err(error) => {
                Self::count("error");
                if self.first_warn(rpc, "eval") {
                    tracing::warn!(
                        error = %error,
                        rpc = %rpc,
                        failure_default,
                        "deja sampler evaluation failed; using configured failure default"
                    );
                }
                failure_policy
            }
        }
    }
}

impl RequestRecordingSampler for SuperpositionRecordingSampler {
    fn should_record(
        &self,
        facts: RequestRecordingFacts,
    ) -> Pin<Box<dyn Future<Output = bool> + Send + '_>> {
        // Evaluation is in-process against the provider's cached snapshot — no
        // network hop, nothing to time out (the RFC's documented deviation from
        // hyperswitch's networked client).
        Box::pin(async move { self.decide(&facts).await })
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

    use std::{path::PathBuf, time::Duration};

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

    /// Writes the policy to a fresh temp file and loads it through the provider.
    /// The file is left in place: the provider WATCHES it, so deleting it under
    /// the watcher is a different test, not fixture cleanup.
    fn policy_file(content: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU64, Ordering};
        static SEQ: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "deja-sampler-test-{}-{}.toml",
            std::process::id(),
            SEQ.fetch_add(1, Ordering::Relaxed),
        ));
        std::fs::write(&path, content).unwrap();
        path
    }

    async fn snapshot(content: &str) -> Arc<SuperpositionConfig> {
        let path = policy_file(content);
        Arc::new(
            SuperpositionConfig::from_file(path.to_str().unwrap())
                .await
                .unwrap(),
        )
    }

    fn sampler_cfg(fail_closed: bool) -> ucs_env::deja_config::SamplerConfig {
        ucs_env::deja_config::SamplerConfig {
            record_key: "deja_record".to_string(),
            fail_closed,
            timeout_ms: 25,
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
    #[tokio::test]
    async fn production_defaults_dark_with_targeted_sample_in() {
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(POLICY).await),
            "production",
            &sampler_cfg(true),
        );
        assert!(
            sampler.decide(&facts(AUTHORIZE)).await,
            "targeted override samples in"
        );
        assert!(
            !sampler.decide(&facts("/types.PaymentService/Refund")).await,
            "everything else inherits the dark default"
        );
    }

    /// The development override keeps the local rig recording wholesale.
    #[tokio::test]
    async fn development_records_everything() {
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(POLICY).await),
            "development",
            &sampler_cfg(true),
        );
        assert!(sampler.decide(&facts(AUTHORIZE)).await);
        assert!(sampler.decide(&facts("/types.PaymentService/Refund")).await);
    }

    /// A snapshot without the record key resolves to the configured failure
    /// default, both ways.
    #[tokio::test]
    async fn missing_key_uses_failure_default_both_ways() {
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
            Some(snapshot(KEYLESS).await),
            "production",
            &sampler_cfg(true),
        );
        assert!(!closed.decide(&facts(AUTHORIZE)).await, "fail-closed skips");
        let open = SuperpositionRecordingSampler::assemble(
            Some(snapshot(KEYLESS).await),
            "production",
            &sampler_cfg(false),
        );
        assert!(open.decide(&facts(AUTHORIZE)).await, "fail-open records");
    }

    /// Record mode with no snapshot at all: every decision is the failure
    /// default (the install site logged the condition).
    #[tokio::test]
    async fn no_source_uses_failure_default() {
        let sampler =
            SuperpositionRecordingSampler::assemble(None, "production", &sampler_cfg(true));
        assert!(!sampler.decide(&facts(AUTHORIZE)).await);
        let open = SuperpositionRecordingSampler::assemble(None, "production", &sampler_cfg(false));
        assert!(open.decide(&facts(AUTHORIZE)).await);
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
    #[tokio::test]
    async fn class_override_targets_the_whole_service() {
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
            Some(snapshot(BY_CLASS).await),
            "production",
            &sampler_cfg(true),
        );
        assert!(
            sampler
                .decide(&facts("/types.PaymentService/Authorize"))
                .await
        );
        assert!(
            sampler
                .decide(&facts("/types.PaymentService/Capture"))
                .await
        );
        assert!(!sampler.decide(&facts("/types.RefundService/Refund")).await);
    }

    /// Percentage sampling: deterministic per request id, boolean wins
    /// wholesale, 0/absent never gates in, 100 always does.
    #[tokio::test]
    async fn percent_gates_deterministically_by_request_id() {
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
            Some(snapshot(PCT).await),
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
        assert!(
            sampler.decide(&with_id(low)).await,
            "bucket below the gate records"
        );
        assert!(
            !sampler.decide(&with_id(high)).await,
            "bucket at/above the gate skips"
        );
        assert!(
            sampler.decide(&with_id(low)).await && !sampler.decide(&with_id(high)).await,
            "same request id, same answer — the gate is deterministic"
        );
    }

    #[tokio::test]
    async fn percent_boundaries_and_boolean_precedence() {
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
            Some(snapshot(EDGES).await),
            "production",
            &sampler_cfg(true),
        );
        assert!(
            production.decide(&facts(AUTHORIZE)).await,
            "percent 100 always records"
        );
        assert!(
            !production
                .decide(&facts("/types.RefundService/Refund"))
                .await,
            "percent 0 (default) never gates in"
        );
        let development = SuperpositionRecordingSampler::assemble(
            Some(snapshot(EDGES).await),
            "development",
            &sampler_cfg(true),
        );
        assert!(
            development.decide(&facts(AUTHORIZE)).await,
            "the wholesale boolean records regardless of percent"
        );
    }

    /// "50 means 50": over a large population of realistic ids, the FNV
    /// bucket splits evenly — the percentage is a rate, not just a gate.
    #[test]
    fn buckets_distribute_uniformly() {
        let n = 10_000u32;
        let below_50 = (0..n)
            .filter(|i| request_bucket(&format!("req-{i:06x}-{}", i * 7919)) < 50)
            .count();
        let below_50 = u32::try_from(below_50).unwrap();
        let fraction = f64::from(below_50) / f64::from(n);
        assert!(
            (0.47..=0.53).contains(&fraction),
            "expected ~50% of ids below bucket 50, got {fraction}"
        );
    }

    /// The point of the provider: a policy edit on disk reaches the sampler
    /// without a restart. Nothing on the sampler side may cache across requests
    /// — the provider's watcher is the only thing that knows the file moved.
    #[tokio::test]
    async fn policy_edit_on_disk_reaches_the_next_decision() {
        let path = policy_file(POLICY);
        let config = Arc::new(
            SuperpositionConfig::from_file(path.to_str().unwrap())
                .await
                .unwrap(),
        );
        let sampler =
            SuperpositionRecordingSampler::assemble(Some(config), "production", &sampler_cfg(true));
        let refund = "/types.PaymentService/Refund";
        assert!(
            !sampler.decide(&facts(refund)).await,
            "before the edit, production refunds are dark"
        );

        tokio::time::sleep(Duration::from_millis(100)).await;
        let mut contents = std::fs::read_to_string(&path).unwrap();
        contents.push_str(
            "\n[[overrides]]\n_context_ = { environment = \"production\", \
             rpc_method = \"/types.PaymentService/Refund\" }\ndeja_record = true\n",
        );
        std::fs::write(&path, contents).unwrap();

        let refreshed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if sampler.decide(&facts(refund)).await {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await;
        let _ = std::fs::remove_file(&path);
        refreshed.expect("the sampler never saw the edited policy");
    }

    /// A persistent policy failure is logged once per rpc, not once per request.
    #[tokio::test]
    async fn failure_warns_are_bounded_per_rpc() {
        let sampler =
            SuperpositionRecordingSampler::assemble(None, "production", &sampler_cfg(true));
        assert!(sampler.first_warn(AUTHORIZE, "eval"), "first failure warns");
        assert!(
            !sampler.first_warn(AUTHORIZE, "eval"),
            "the same failure again is silent"
        );
        assert!(
            sampler.first_warn("/types.PaymentService/Refund", "eval"),
            "a different rpc warns on its own"
        );
        assert!(
            sampler.first_warn(AUTHORIZE, "key"),
            "a different failure kind on the same rpc warns"
        );
    }
}
