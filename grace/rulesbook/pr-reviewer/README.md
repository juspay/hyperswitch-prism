# PR Reviewer

Portable, repo-owned PR review prompts and rules for `hyperswitch-prism`.

This folder is designed to work with Claude, OpenCode, Cursor, Codex, or any other coding tool that can read files, inspect diffs, and optionally spawn nested subagents. The policy lives here in plain Markdown and YAML so the review behavior stays consistent even when the execution tool changes.

## What This Does

- Reviews any PR in `hyperswitch-prism` with a strict, fail-closed posture.
- Uses a nested-subagent workflow: orchestrator -> classifier -> scenario reviewers -> aggregator.
- Uses scenario-specific subagents as the default dispatch unit, so each PR is reviewed by the exact scenario agents it matches.
- Classifies mixed PRs across connector, core, flow, proto, server, SDK, tests, docs, and CI/security scenarios.
- Forces companion-file checks so the review does not stop at changed files alone.
- Treats PR quality strictly: missing evidence, stale generated outputs, schema drift, security regressions, and partial implementations are blocking concerns.
- Keeps comments code-only: source files, tests, specs, generated artifacts, and required companion files only.

## Folder Layout

- `config/path-rules.yaml` - repo-specific classification rules, scenario taxonomy, companion files, and owner hints.
- `config/rubric.yaml` - strict review policy, severity rules, CI gates, and scenario checklists.
- `config/output-template.md` - required final review structure.
- `prompts/orchestrator.md` - top-level controller for full PR review.
- `prompts/classifier.md` - maps a PR to one or more repo-specific scenarios.
- `prompts/aggregator.md` - normalizes findings and decides the final verdict.
- `subagents/*.md` - primary scenario-specific subagents; one runs per matched scenario.
- `reviewers/*.md` - specialized reviewer prompts for the major repo domains.
- `workflows/full-pr-review.md` - default end-to-end workflow.
- `workflows/grace-pr-review.md` - specialized entrypoint for PRs raised by `GRACE`.
- `workflows/batch-grace-pr-review.md` - batch workflow for all open `GRACE` PRs.
- `workflows/incremental-review.md` - follow-up workflow after author updates.

## Core Principles

- Read every changed file fully.
- Never approve from PR title, summary, or selective snippets.
- Always classify first, then review with the matching scenario specialists.
- Keep comments anchored to code and required companion files.
- Escalate mixed PRs instead of flattening them into one generic review.
- Prefer blocking over guessing when safety cannot be verified from evidence.

## Quick Start

Use the same entry prompt in any coding tool.

### Full review by PR URL

```text
Review PR https://github.com/juspay/hyperswitch-prism/pull/123.
Read grace/rulesbook/pr-reviewer/workflows/full-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

### Full review by PR number

```text
Review PR #123 in hyperswitch-prism.
Read grace/rulesbook/pr-reviewer/workflows/full-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

### Incremental review after updates

```text
Re-review PR #123 in hyperswitch-prism after the latest commits.
Read grace/rulesbook/pr-reviewer/workflows/incremental-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

### Review a PR raised by `GRACE`

```text
Review PR #123 in hyperswitch-prism.
This PR was raised by GRACE / GRACE automation.
Read grace/rulesbook/pr-reviewer/workflows/grace-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

## Required Inputs

At minimum, the runner should provide one of:

- a PR URL
- a PR number
- base/head refs plus the full diff

The review should also collect:

- PR author/login
- head repository owner/name
- changed file list with statuses
- full unified diff
- PR title and body only when they help infer code scope

## Output Contract

The final result must follow `grace/rulesbook/pr-reviewer/config/output-template.md` and include:

- final verdict
- PR classification
- areas reviewed
- blocking findings
- warnings and non-blocking gaps
- missing companion changes
- suggested code fixes

## Scenario Coverage

This reviewer is intentionally scenario-aware. The default taxonomy covers:

1. connector new integration
2. connector flow addition
3. connector payment method addition
4. connector bugfix, webhook, dispute, or auth change
5. connector shared plumbing
6. core shared domain or trait change
7. new flow or framework-wide flow change
8. proto or API contract change
9. gRPC server or composite orchestration change
10. SDK, FFI, or generated code change
11. tests, specs, harness, or docs change
12. CI, config, release, security, or credential-sensitive change
13. PR raised by `GRACE` / generated by GRACE automation

## Repo Anchors

The rules here are derived from real repo structure and guards, including:

- `.github/workflows/ci.yml`
- `.github/workflows/pr-convention-checks.yml`
- `.github/workflows/sdk-client-sanity.yml`
- `Cargo.toml`
- `Makefile`
- `crates/integrations/connector-integration/src/connectors.rs`
- `crates/integrations/connector-integration/src/default_implementations.rs`
- `crates/integrations/connector-integration/src/types.rs`
- `crates/types-traits/domain_types/src/connector_flow.rs`
- `crates/types-traits/interfaces/src/connector_types.rs`
- `crates/types-traits/grpc-api-types/build.rs`

## Usage Notes

- If your tool supports subagents, spawn them exactly as described in the workflow docs.
- The preferred mode is one subagent per matched scenario plus any metadata subagents.
- If your tool does not support subagents, run the same steps serially and keep the same boundaries.
- If the PR is raised by `GRACE`, always run the dedicated `grace-generated-pr` code-pattern reviewer in addition to the code-domain reviewers.
- Do not comment on labels, approvals, PR title, branch naming, or CI status.
- Do not turn this into a praise bot. The target behavior is strict, evidence-driven review.
