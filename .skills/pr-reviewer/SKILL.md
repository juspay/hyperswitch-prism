---
name: pr-reviewer
description: >
  Reviews pull requests in the hyperswitch-prism (UCS) Rust codebase using a strict,
  fail-closed, scenario-aware review system. Classifies PRs into connector, core-flow,
  proto, server, SDK, CI/security, and GRACE-generated scenarios, then dispatches
  specialist subagents per scenario. Use when reviewing any PR, batch-reviewing open
  GRACE PRs, or re-reviewing after author updates.
license: Apache-2.0
compatibility: Requires git and gh CLI. Works with any AI coding tool that can read files and inspect diffs.
metadata:
  author: parallal
  version: "1.0"
  domain: code-review
---

# PR Reviewer

## Overview

This skill reviews pull requests in `hyperswitch-prism` with a strict, evidence-driven,
code-only posture. It uses a nested-subagent workflow:

**orchestrator -> classifier -> scenario subagents -> aggregator**

Each PR is classified into one or more of 13 repo-specific scenarios, then reviewed by
the exact specialist subagents that match. The review covers changed files, required
companion files, and produces a structured verdict with blocking/non-blocking findings.

## When to Use

- User asks to **review a PR** (by URL, number, or diff)
- User asks to **review all open GRACE PRs** in batch
- User asks to **re-review a PR** after author updates
- User asks to **review a PR raised by 10xGRACE**

## Workflows

Choose the workflow based on the request:

| Request | Workflow | Path |
|---------|----------|------|
| Review any PR | Full PR Review | `grace/rulesbook/pr-reviewer/workflows/full-pr-review.md` |
| Review a GRACE-authored PR | GRACE PR Review | `grace/rulesbook/pr-reviewer/workflows/grace-pr-review.md` |
| Batch review all open GRACE PRs | Batch GRACE Review | `grace/rulesbook/pr-reviewer/workflows/batch-grace-pr-review.md` |
| Re-review after updates | Incremental Review | `grace/rulesbook/pr-reviewer/workflows/incremental-review.md` |

**To execute a review:** Read the selected workflow file and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.

## Quick Start

### Full review by PR URL

```text
Review PR https://github.com/juspay/hyperswitch-prism/pull/123.
Read grace/rulesbook/pr-reviewer/workflows/full-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

### Review a PR raised by 10xGRACE

```text
Review PR #123 in hyperswitch-prism.
This PR was raised by 10xGRACE / GRACE automation.
Read grace/rulesbook/pr-reviewer/workflows/grace-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

### Batch review all open GRACE PRs

```text
Review all open GRACE PRs in hyperswitch-prism.
Read grace/rulesbook/pr-reviewer/workflows/batch-grace-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

### Incremental review after updates

```text
Re-review PR #123 in hyperswitch-prism after the latest commits.
Read grace/rulesbook/pr-reviewer/workflows/incremental-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

## Review System Architecture

### Phase 0 -- Load Configuration
Read these files before any review:
- `grace/rulesbook/pr-reviewer/README.md` -- system overview and principles
- `grace/rulesbook/pr-reviewer/config/path-rules.yaml` -- scenario taxonomy, path triggers, companion files
- `grace/rulesbook/pr-reviewer/config/rubric.yaml` -- severity levels, decision rules, scenario checklists
- `grace/rulesbook/pr-reviewer/config/output-template.md` -- required output structure
- `grace/rulesbook/pr-reviewer/prompts/orchestrator.md` -- top-level controller

### Phase 1 -- Classify
Run `grace/rulesbook/pr-reviewer/prompts/classifier.md` to determine:
- Primary and secondary scenarios
- Metadata scenarios (e.g., grace-generated-pr)
- Subagents and reviewer families to dispatch
- Companion files to check

### Phase 2 -- Review
Spawn one subagent per matched scenario from `grace/rulesbook/pr-reviewer/subagents/`.
Fall back to family reviewers in `grace/rulesbook/pr-reviewer/reviewers/` if no dedicated subagent exists.

### Phase 3 -- Aggregate
Run `grace/rulesbook/pr-reviewer/prompts/aggregator.md` to deduplicate findings,
normalize severities, and produce the final verdict using `grace/rulesbook/pr-reviewer/config/output-template.md`.

## Scenario Coverage

| # | Scenario | Subagent |
|---|----------|----------|
| 1 | Connector new integration | `grace/rulesbook/pr-reviewer/subagents/connector-new-integration.md` |
| 2 | Connector flow addition | `grace/rulesbook/pr-reviewer/subagents/connector-flow-addition.md` |
| 3 | Connector payment method addition | `grace/rulesbook/pr-reviewer/subagents/connector-payment-method-addition.md` |
| 4 | Connector bugfix / webhook / dispute | `grace/rulesbook/pr-reviewer/subagents/connector-bugfix-webhook.md` |
| 5 | Connector shared plumbing | `grace/rulesbook/pr-reviewer/subagents/connector-shared-plumbing.md` |
| 6 | Core flow / framework change | `grace/rulesbook/pr-reviewer/subagents/core-flow-framework.md` |
| 7 | Proto / API contract change | `grace/rulesbook/pr-reviewer/subagents/proto-api-contract.md` |
| 8 | gRPC server / composite orchestration | `grace/rulesbook/pr-reviewer/subagents/server-composite.md` |
| 9 | SDK / FFI / codegen change | `grace/rulesbook/pr-reviewer/subagents/sdk-ffi-codegen.md` |
| 10 | Tests / specs / docs change | `grace/rulesbook/pr-reviewer/subagents/tests-specs-docs.md` |
| 11 | CI / config / security change | `grace/rulesbook/pr-reviewer/subagents/ci-config-security.md` |
| 12 | GRACE tooling change | `grace/rulesbook/pr-reviewer/subagents/grace-tooling.md` |
| 13 | GRACE-generated PR (metadata) | `grace/rulesbook/pr-reviewer/subagents/grace-generated-pr.md` |

## Reviewer Families

| Family | Prompt |
|--------|--------|
| connector | `grace/rulesbook/pr-reviewer/reviewers/connector.md` |
| core-flow | `grace/rulesbook/pr-reviewer/reviewers/core-flow.md` |
| proto-api | `grace/rulesbook/pr-reviewer/reviewers/proto-api.md` |
| server-composite | `grace/rulesbook/pr-reviewer/reviewers/server-composite.md` |
| sdk-ffi-codegen | `grace/rulesbook/pr-reviewer/reviewers/sdk-ffi-codegen.md` |
| tests-docs | `grace/rulesbook/pr-reviewer/reviewers/tests-docs.md` |
| ci-config-security | `grace/rulesbook/pr-reviewer/reviewers/ci-config-security.md` |
| grace-generated-pr | `grace/rulesbook/pr-reviewer/reviewers/grace-generated-pr.md` |

## Core Principles

- Read every changed file fully -- never approve from title, summary, or snippets
- Classify first, then review with matching scenario specialists
- Keep comments anchored to code and required companion files only
- Escalate mixed PRs instead of flattening into one generic review
- Fail closed when safety cannot be verified from evidence

## Reference Index

| Path | Contents |
|------|----------|
| `grace/rulesbook/pr-reviewer/README.md` | System overview and principles |
| `grace/rulesbook/pr-reviewer/config/path-rules.yaml` | Scenario taxonomy, path triggers, companion files |
| `grace/rulesbook/pr-reviewer/config/rubric.yaml` | Severity levels, decision rules, scenario checklists |
| `grace/rulesbook/pr-reviewer/config/output-template.md` | Required output structure |
| `grace/rulesbook/pr-reviewer/prompts/orchestrator.md` | Top-level review controller |
| `grace/rulesbook/pr-reviewer/prompts/classifier.md` | PR-to-scenario classifier |
| `grace/rulesbook/pr-reviewer/prompts/aggregator.md` | Findings aggregator and verdict producer |
| `grace/rulesbook/pr-reviewer/subagents/` | Scenario-specific subagent prompts (13 scenarios) |
| `grace/rulesbook/pr-reviewer/reviewers/` | Family reviewer prompts (8 families) |
| `grace/rulesbook/pr-reviewer/workflows/` | Entry-point workflows (4 workflows) |
