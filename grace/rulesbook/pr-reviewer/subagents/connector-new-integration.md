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
- proof that the PR is truly connector-scoped

## Extra Checks

- all registration points are present
- connector naming is consistent across files and branch/title when applicable
- unsupported flows are explicit
- no copied provider logic survives with wrong endpoints or semantics

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/connector.md`.
