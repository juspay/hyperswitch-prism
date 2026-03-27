# Full PR Review Workflow

This is the default workflow for reviewing any PR in `connector-service`.

## Goal

Produce one strict, scenario-aware review that inspects every changed file and uses nested subagents when possible.

## Step 0: Load the review system

Read these first:

- `grace/rulesbook/pr-reviewer/README.md`
- `grace/rulesbook/pr-reviewer/config/path-rules.yaml`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`
- `grace/rulesbook/pr-reviewer/config/output-template.md`
- `grace/rulesbook/pr-reviewer/prompts/orchestrator.md`

## Step 1: Gather PR context

Resolve the PR URL or number and collect:

- PR author/login
- head repository owner/name
- changed file list with status
- full unified diff
- PR title and body only when they help classify code scope

If you are using GitHub tooling, prefer `gh` for PR diff and file metadata.

## Step 2: Run the classifier

Run `grace/rulesbook/pr-reviewer/prompts/classifier.md`.

Expected output:

- primary scenario
- secondary scenarios
- metadata scenarios used only for scenario activation
- scenario subagents
- reviewer families
- mixed PR risk
- companion files to read

## Step 3: Spawn reviewer subagents

If the tool supports nested subagents, spawn one subagent per matched scenario using the `SCENARIO_SUBAGENTS` returned by the classifier.

Default scenario subagents live in `grace/rulesbook/pr-reviewer/subagents/`.

If a scenario has no dedicated subagent or needs a second pass, use the family reviewers in `grace/rulesbook/pr-reviewer/reviewers/`.

Useful fallback reviewers:

- `grace/rulesbook/pr-reviewer/reviewers/connector.md`
- `grace/rulesbook/pr-reviewer/reviewers/core-flow.md`
- `grace/rulesbook/pr-reviewer/reviewers/proto-api.md`
- `grace/rulesbook/pr-reviewer/reviewers/server-composite.md`
- `grace/rulesbook/pr-reviewer/reviewers/sdk-ffi-codegen.md`
- `grace/rulesbook/pr-reviewer/reviewers/tests-docs.md`
- `grace/rulesbook/pr-reviewer/reviewers/ci-config-security.md`
- `grace/rulesbook/pr-reviewer/reviewers/grace-generated-pr.md`

Each reviewer should receive:

- normalized PR review packet
- classifier output
- assigned file subset
- companion files

If `grace-generated-pr` appears in `METADATA_SCENARIOS`, the `grace/rulesbook/pr-reviewer/reviewers/grace-generated-pr.md` reviewer is mandatory.

If the tool does not support subagents, execute the same reviewers serially yourself.

## Step 4: Aggregate

Run `grace/rulesbook/pr-reviewer/prompts/aggregator.md` and produce the final result using `grace/rulesbook/pr-reviewer/config/output-template.md`.

## Step 5: Final answer

The final answer must include:

- verdict
- classification
- blocking code findings
- non-blocking code findings
- missing code companion changes
- suggested code fixes

## Portable Invocation Prompt

Use this in Claude, OpenCode, Cursor, Codex, or any similar tool:

```text
Review PR <PR-REFERENCE> in connector-service.
Read grace/rulesbook/pr-reviewer/workflows/full-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```
