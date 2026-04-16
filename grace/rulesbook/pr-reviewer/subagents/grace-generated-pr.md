# Grace Generated Code Pattern Subagent

Review a PR classified as `grace-generated-pr`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/grace-generated-pr.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- copied/generated connector code that is not fully provider-specific
- flow or payment-method scope drift in match arms and guards
- incomplete registration, transformer, test, or spec wiring
- brittle raw parsing, manual serialization, or placeholder code left in production paths

## Extra Checks

- ignore labels, approvals, PR title, branch names, CI status, and PR-body/process commentary
- keep every comment anchored to code or required companion files
- treat widened support without explicit guards as a likely blocker

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/grace-generated-pr.md`.
