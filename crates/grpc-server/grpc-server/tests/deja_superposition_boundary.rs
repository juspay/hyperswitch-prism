//! The Superposition read boundary under REPLAY — the graceful-miss contract.
//!
//! In replay there is no workspace to consult. A config read that was never recorded
//! (a novel read) must degrade to a recoverable `Err` so `resolve_connector_urls`
//! falls back to static config and the replayed request progresses — it must NOT
//! fail-stop and it must NOT serve whatever the local snapshot happens to hold. The
//! sampler's own `resolve_with` is deliberately not a boundary (it runs in record
//! mode only), so under the same replay hook it still resolves directly.
//!
//! Own test binary: `set_global_runtime_hook` latches once per process.
#![cfg(feature = "deja")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use common_utils::{superposition_config::SuperpositionConfigError, SuperpositionConfig};
use ucs_env::configs;

fn baked_path() -> String {
    format!(
        "{}/config/superposition.toml",
        configs::workspace_path().display()
    )
}

/// Install a REPLAY hook whose lookup table is empty: every boundary lookup misses.
fn install_empty_replay_hook() {
    let table = deja::LookupTable {
        recording_id: "superposition-boundary-test".to_string(),
        policy_version: 1,
        entries: vec![],
    };
    let path = std::env::temp_dir().join(format!(
        "deja-superposition-boundary-{}.json",
        std::process::id()
    ));
    std::fs::write(&path, serde_json::to_vec(&table).unwrap()).unwrap();
    let hook = deja::LookupTableHook::from_source(
        deja::LocalFileLookupSource::new(path),
        deja::InMemoryObservedSink::new(),
    )
    .expect("lookup hook");
    deja::set_global_runtime_hook(Some(deja::RuntimeHook::LookupReplay(hook)))
        .expect("install runtime hook");
}

#[tokio::test]
async fn replay_miss_degrades_to_a_recoverable_error_and_never_reads_the_snapshot() {
    install_empty_replay_hook();
    let config = SuperpositionConfig::from_file(&baked_path()).await.unwrap();

    // The connector-URL read is a boundary: a novel read in replay is a MISS, and a
    // miss is a typed Err — the caller's static-config fallback — not a panic and not
    // the baked file's answer.
    let error = config
        .resolve("stripe", "sandbox")
        .await
        .expect_err("a novel config read in replay must miss, not resolve from the local snapshot");
    assert!(
        matches!(error, SuperpositionConfigError::ResolutionError(ref message) if message.contains("novel config read")),
        "unexpected error: {error}"
    );

    // The sampler's read is NOT a boundary (record mode only): under the same replay
    // hook it resolves directly from the provider.
    let resolved = config
        .resolve_with(&[("connector", "stripe"), ("environment", "sandbox")], None)
        .await
        .expect("resolve_with is not a boundary and must resolve under replay");
    assert_eq!(
        resolved
            .get("connector_base_url")
            .and_then(|url| url.as_str()),
        Some("https://api.stripe.com/")
    );
}
