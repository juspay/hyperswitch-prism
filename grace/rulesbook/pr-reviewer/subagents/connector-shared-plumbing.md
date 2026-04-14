# Connector Shared Plumbing Subagent

Review a PR classified as `connector-shared-plumbing`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/connector.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- connector registry changes
- shared helper, macro, webhook utility, and cross-connector behavior changes
- blast radius across multiple connectors
- correctness of fallback/default implementations

## Extra Checks

- a shared helper change is not validated by only one connector
- registry or macro changes do not silently break existing connectors
- mixed connector/core fallout is surfaced explicitly

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/connector.md`.
