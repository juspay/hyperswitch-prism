# Server Composite Subagent

Review a PR classified as `server-composite`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/server-composite.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- request validation and routing
- orchestration, timeout, retry, and partial failure behavior
- error propagation and response shaping
- contract alignment with domain and proto layers

## Extra Checks

- no silent fallback or success-like failure handling
- no new flow is partially wired
- composite-service changes are validated at the right layer

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/server-composite.md`.
