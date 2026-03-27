# Review Output Template

Use this structure for the final synthesized review.

The review must stay code-only. Comment only on source files, tests, specs, generated artifacts, and required companion files that affect the changed code.

## Verdict

- Decision: `approve` | `comment` | `request_changes`
- Risk: `low` | `medium` | `high` | `critical`
- Primary scenario: `<one scenario id>`
- Secondary scenarios: `<comma-separated list or none>`
- Code summary: `<one-line statement of the core code change>`

## Scope

- Files reviewed: `<all changed files covered>`
- Areas reviewed: `<connector | core-flow | proto-api | server-composite | sdk-ffi-codegen | tests-docs | ci-config-security | grace-generated-pr>`
- Core code paths inspected: `<important functions, modules, or generated files>`
- Companion code files inspected: `<tests/specs/registries/traits/generated outputs>`

## Blocking Code Findings

- `[S0|S1] <title> - <why it matters> - <file path>`
- `[S0|S1] <title> - <why it matters> - <file path>`

If there are no blockers, write `- none`.

## Non-Blocking Code Findings

- `[S2|S3] <title> - <why it matters> - <file path>`
- `[S2|S3] <title> - <why it matters> - <file path>`

If there are no warnings, write `- none`.

## Missing Code Companions

- `<missing registration, test, spec, enum, trait, or generated file update>`
- `<missing registration, test, spec, enum, trait, or generated file update>`

If there are no gaps, write `- none`.

## Suggested Code Fixes

- `<one concise code-level fix>`
- `<one concise code-level fix>`

If there are no fixes to suggest, write `- none`.

## Style Rules

- Cite concrete file paths.
- Prefer code findings over praise.
- Do not mention labels, approvals, PR title, PR body, CI status, branch naming, or owner/process state.
- Do not report vague concerns without code evidence.
- Merge duplicate findings into one root-cause statement.
- If the diff is too broad to review safely, say so in code terms.
