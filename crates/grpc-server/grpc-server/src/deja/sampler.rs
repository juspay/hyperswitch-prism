//! Per-request recording sampler: the seam AND the Superposition-backed policy.
//!
//! The déjà library does not ship the trait (it is the integrator's policy), so it is
//! declared here, together with [`SuperpositionRecordingSampler`] — the concrete policy a
//! record-mode process installs. The decision (`deja_record`, default FALSE) is evaluated
//! in-process against the boot-parsed `config/superposition.toml`, dimensioned on
//! `environment` × `rpc_method`, memoized per rpc (the snapshot is frozen at boot, so a
//! memo can never go stale), and fail-closed: every failure path — no snapshot, key
//! missing, eval error — resolves to `!fail_closed`, which by default records nothing.
//!

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

/// The Superposition-backed recording policy (see the module doc). Built once at
/// install time, exactly when the process is in record mode.
pub struct SuperpositionRecordingSampler {
    /// `None` = record mode booted with no superposition snapshot: the no-source
    /// state, where every decision is `!fail_closed` (the install site logged it).
    superposition: Option<Arc<SuperpositionConfig>>,
    environment: &'static str,
    record_key: String,
    fail_closed: bool,
    /// rpc path → decision. Sound because the snapshot is frozen at boot; failure
    /// decisions memoize too, which also bounds their warn to once per rpc.
    memo: std::sync::RwLock<HashMap<String, bool>>,
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
            record_key: sampler.record_key.clone(),
            fail_closed: sampler.fail_closed,
            memo: std::sync::RwLock::new(HashMap::new()),
        }
    }

    fn decide(&self, facts: &RequestRecordingFacts) -> bool {
        if let Some(hit) = self
            .memo
            .read()
            .ok()
            .and_then(|memo| memo.get(&facts.rpc).copied())
        {
            return hit;
        }

        let failure_default = !self.fail_closed;
        let decision = match &self.superposition {
            None => failure_default, // no-source: logged once at install
            Some(superposition) => match superposition.resolve_with(&[
                ("environment", self.environment),
                ("rpc_method", &facts.rpc),
            ]) {
                Ok(resolved) => match resolved.get(&self.record_key).and_then(|v| v.as_bool()) {
                    Some(decision) => decision,
                    None => {
                        tracing::warn!(
                            record_key = %self.record_key,
                            rpc = %facts.rpc,
                            failure_default,
                            "deja sampler key missing or non-boolean in superposition snapshot; \
                             using configured failure default"
                        );
                        failure_default
                    }
                },
                Err(error) => {
                    tracing::warn!(
                        error = %error,
                        rpc = %facts.rpc,
                        failure_default,
                        "deja sampler evaluation failed; using configured failure default"
                    );
                    failure_default
                }
            },
        };
        if let Ok(mut memo) = self.memo.write() {
            memo.insert(facts.rpc.clone(), decision);
        }
        decision
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

    /// Decisions memoize per rpc — sound because the snapshot is boot-frozen.
    #[test]
    fn memoizes_per_rpc() {
        let sampler = SuperpositionRecordingSampler::assemble(
            Some(snapshot(POLICY)),
            "production",
            &sampler_cfg(true),
        );
        sampler.decide(&facts(AUTHORIZE));
        assert_eq!(
            sampler.memo.read().unwrap().get(AUTHORIZE).copied(),
            Some(true),
            "first consult lands in the memo"
        );
    }


}
