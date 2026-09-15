// Fixture assertions may unwrap/expect freely — a panic IS the test failing.
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::{fs, time::Duration};

use common_utils::{
    superposition_config::{SuperpositionClientConfig, SuperpositionSource},
    SuperpositionConfig,
};
use tokio::time::{sleep, timeout};
use ucs_env::configs;

/// `config/superposition.toml` is validated against its own `[dimensions]` schema at parse
/// time — every `[[overrides]] _context_` value must appear in the matching dimension's
/// `enum` list. A single mismatched value (e.g. a connector added via an override without
/// also being added to `[dimensions].connector`'s enum) fails parsing for the *entire*
/// file, which silently disables superposition URL overrides for every connector at
/// runtime (falls back to static config with no fatal error). This test exists so that
/// class of mistake fails CI instead of degrading production silently.
#[tokio::test]
async fn superposition_toml_loads_and_resolves_successfully() {
    let path = format!(
        "{}/config/superposition.toml",
        configs::workspace_path().display()
    );
    let config = SuperpositionConfig::from_file(&path)
        .await
        .unwrap_or_else(|e| {
            panic!(
                "config/superposition.toml failed to parse: {e}\n\
             Check that every `_context_` value used in an [[overrides]] block is present \
             in the corresponding dimension's `enum` list under [dimensions]."
            )
        });
    let resolved = config.resolve("stripe", "sandbox").await.unwrap();
    assert_eq!(
        resolved
            .get("connector_base_url")
            .and_then(|url| url.as_str()),
        Some("https://api.stripe.com/")
    );
}

#[tokio::test]
async fn superposition_toml_file_changes_are_reloaded() {
    let source_path = configs::workspace_path().join("config/superposition.toml");
    let temp_path = std::env::temp_dir().join(format!(
        "prism-superposition-watch-{}.toml",
        std::process::id()
    ));
    fs::copy(&source_path, &temp_path).unwrap();

    let config = SuperpositionConfig::from_file(temp_path.to_str().unwrap())
        .await
        .unwrap();
    let initial = config.resolve("stripe", "sandbox").await.unwrap();
    assert_eq!(
        initial
            .get("connector_base_url")
            .and_then(|url| url.as_str()),
        Some("https://api.stripe.com/")
    );
    sleep(Duration::from_millis(100)).await;

    let contents = fs::read_to_string(&temp_path).unwrap();
    let updated = contents.replacen(
        "connector_base_url = \"https://api.stripe.com/\"",
        "connector_base_url = \"https://updated.stripe.test/\"",
        1,
    );
    assert_ne!(contents, updated);
    fs::write(&temp_path, updated).unwrap();

    let refreshed = timeout(Duration::from_secs(5), async {
        loop {
            let resolved = config.resolve("stripe", "sandbox").await.unwrap();
            if resolved
                .get("connector_base_url")
                .and_then(|url| url.as_str())
                == Some("https://updated.stripe.test/")
            {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    })
    .await;

    let _ = fs::remove_file(temp_path);
    refreshed.expect("superposition config was not refreshed after the file changed");
}

fn baked_path() -> String {
    format!(
        "{}/config/superposition.toml",
        configs::workspace_path().display()
    )
}

fn remote_settings() -> SuperpositionClientConfig {
    SuperpositionClientConfig {
        source: SuperpositionSource::Remote,
        // Nothing listens here: connection refused, immediately. The point is a
        // remote source that cannot initialise, so the fallback path is exercised
        // for real instead of through a stub.
        endpoint: "http://127.0.0.1:9".to_string(),
        token: hyperswitch_masking::Secret::new("sp_test".to_string()),
        org_id: "hyperswitch".to_string(),
        workspace_id: "prism".to_string(),
        ..SuperpositionClientConfig::default()
    }
}

/// Source selection, `file`: the baked file, watched — today's behaviour.
#[tokio::test]
async fn file_settings_select_the_baked_file() {
    let config = SuperpositionConfig::new(&SuperpositionClientConfig::default(), &baked_path())
        .await
        .unwrap();
    assert_eq!(config.source(), SuperpositionSource::File);
    assert!(!config.experiments_supported());
    let resolved = config.resolve("stripe", "sandbox").await.unwrap();
    assert_eq!(
        resolved
            .get("connector_base_url")
            .and_then(|url| url.as_str()),
        Some("https://api.stripe.com/")
    );
}

/// Source selection, `remote` but the workspace is unreachable: the provider
/// initialises from its fallback file (hyperswitch's `backup_file_path`
/// contract) and stays a REMOTE provider — polling keeps trying the workspace
/// — while serving the file's policy meanwhile.
#[tokio::test]
async fn unreachable_remote_initialises_from_the_fallback_file() {
    let config = SuperpositionConfig::new(&remote_settings(), &baked_path())
        .await
        .unwrap();
    assert_eq!(config.source(), SuperpositionSource::Remote);
    assert!(config.experiments_supported());
    let resolved = config.resolve("stripe", "sandbox").await.unwrap();
    assert_eq!(
        resolved
            .get("connector_base_url")
            .and_then(|url| url.as_str()),
        Some("https://api.stripe.com/"),
        "the fallback file's policy is what the remote provider serves until the workspace answers"
    );
}

/// The targeting key is the experiment-bucketing identifier, not a dimension: on a
/// source that carries no experiments it changes nothing, and it never leaks into
/// the dimension match (a policy keyed on `environment`/`connector` resolves the
/// same with or without it).
#[tokio::test]
async fn targeting_key_is_inert_without_experiments_and_is_not_a_dimension() {
    let config = SuperpositionConfig::from_file(&baked_path()).await.unwrap();
    let dims = [("connector", "stripe"), ("environment", "sandbox")];
    let without = config.resolve_with(&dims, None).await.unwrap();
    let with = config
        .resolve_with(&dims, Some("request-42"))
        .await
        .unwrap();
    assert_eq!(without, with);
    assert_eq!(
        with.get("connector_base_url").and_then(|url| url.as_str()),
        Some("https://api.stripe.com/")
    );
}

/// Both the workspace and the configured fallback file are unusable: boot still
/// continues on the baked file — fail-open — rather than aborting like hyperswitch.
#[tokio::test]
async fn unreachable_remote_with_bad_fallback_degrades_to_the_baked_file() {
    let settings = SuperpositionClientConfig {
        backup_file_path: Some(std::path::PathBuf::from("/nonexistent/superposition.toml")),
        ..remote_settings()
    };
    let config = SuperpositionConfig::new(&settings, &baked_path())
        .await
        .unwrap();
    assert_eq!(config.source(), SuperpositionSource::File);
}
