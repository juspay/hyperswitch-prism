#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

use common_utils::SuperpositionConfig;
use ucs_env::configs;

/// `config/superposition.toml` is validated against its own `[dimensions]` schema at parse
/// time — every `[[overrides]] _context_` value must appear in the matching dimension's
/// `enum` list. A single mismatched value (e.g. a connector added via an override without
/// also being added to `[dimensions].connector`'s enum) fails parsing for the *entire*
/// file, which silently disables superposition URL overrides for every connector at
/// runtime (falls back to static config with no fatal error). This test exists so that
/// class of mistake fails CI instead of degrading production silently.
#[test]
fn superposition_toml_parses_successfully() {
    let path = format!(
        "{}/config/superposition.toml",
        configs::workspace_path().display()
    );
    SuperpositionConfig::from_file(&path).unwrap_or_else(|e| {
        panic!(
            "config/superposition.toml failed to parse: {e}\n\
             Check that every `_context_` value used in an [[overrides]] block is present \
             in the corresponding dimension's `enum` list under [dimensions]."
        )
    });
}
