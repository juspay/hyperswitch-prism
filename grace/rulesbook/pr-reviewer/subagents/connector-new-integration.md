# Connector New Integration Subagent

Review a PR classified as `connector-new-integration`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/connector.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- new connector file creation and transformer layout
- registration in `connectors.rs`
- companion updates in `default_implementations.rs`, `types.rs`, and `ConnectorEnum` plumbing
- connector-specific auth, URLs, status mapping, and error mapping
- superposition URL registration (`config/superposition.toml`) and dynamic URL patching (`types.rs`)
- proof that the PR is truly connector-scoped

## Extra Checks

- all registration points are present
- connector naming is consistent across files and branch/title when applicable
- unsupported flows are explicit
- no copied provider logic survives with wrong endpoints or semantics
- **superposition URLs registered**: `config/superposition.toml` adds the connector to the
  `connector` dimension `enum` AND has `connector_base_url` override blocks for sandbox (default)
  and production
- **dynamic URL patching wired**: `crates/types-traits/domain_types/src/types.rs` `Connectors::apply()`
  has a `ConnectorEnum::<Variant>` match arm (not left to the `_ =>` fallback) and the connector is
  added to the fallback "Supported connectors:" list. Flag a new connector that skips either edit —
  it silently ships without dynamic URL patching. Reference: PR #2118.

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/connector.md`.
