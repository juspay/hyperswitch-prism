# Connector Bugfix Webhook Subagent

Review a PR classified as `connector-bugfix-webhook`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/connector.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- auth and request signing fixes
- webhook parsing, verification, and source validation
- redirect, dispute, refund, or sync bugfixes
- regression risk in already-supported flows

## Extra Checks

- security-sensitive logic is not weakened
- bugfix scope matches test coverage and PR description
- no secrets or raw headers leak through diagnostics or PR evidence

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/connector.md`.
