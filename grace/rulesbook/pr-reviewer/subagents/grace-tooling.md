# Grace Tooling Subagent

Review a PR classified as `grace-tooling`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/ci-config-security.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- GRACE workflow, rulesbook, or automation changes
- prompt and workflow contract drift
- codegen safety, quality gate expectations, and PR creation semantics

## Extra Checks

- GRACE changes do not weaken the repository's review or quality model
- workflow changes remain consistent with documented behavior
- PR-generation logic does not produce misleading metadata or unsafe outputs

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/ci-config-security.md`.
