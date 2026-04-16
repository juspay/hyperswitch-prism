# Core Flow Framework Subagent

Review a PR classified as `core-flow-framework`.

## Read First

- `grace/rulesbook/pr-reviewer/reviewers/core-flow.md`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Focus

- shared domain types, traits, enums, helpers, and flow markers
- blast radius across connectors, server, SDK, and codegen
- backward compatibility and semantic stability

## Extra Checks

- trait changes have a complete downstream story
- enum or request/response semantic changes are explicitly justified
- refactor framing is not hiding behavioral change

## Output

Use the standard structured finding format from `grace/rulesbook/pr-reviewer/reviewers/core-flow.md`.
