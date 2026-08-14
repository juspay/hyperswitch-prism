#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use std::{fs, time::Duration};

use common_utils::SuperpositionConfig;
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
