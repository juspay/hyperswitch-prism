# GRACE PR Review Workflow

Use this workflow for PRs raised by `10xGRACE`.

## Goal

Run the normal code review plus a mandatory GRACE-specific code-pattern pass.

The final review must stay code-only.

## Step 0: Load the review system

Read these first:

- `grace/rulesbook/pr-reviewer/README.md`
- `grace/rulesbook/pr-reviewer/config/path-rules.yaml`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`
- `grace/rulesbook/pr-reviewer/config/output-template.md`
- `grace/rulesbook/pr-reviewer/prompts/orchestrator.md`
- `grace/rulesbook/pr-reviewer/subagents/grace-generated-pr.md`

## Step 1: Gather code review inputs

Resolve the PR and collect:

- PR author/login or head repository owner/name to detect `10xGRACE`
- changed file list with status
- full unified diff
- code files and required companion files

## Step 2: Confirm this is a GRACE-raised PR

Treat it as GRACE-raised if the PR author or head repo owner is `10xGRACE`.

If not, fall back to `grace/rulesbook/pr-reviewer/workflows/full-pr-review.md`.

## Step 3: Run the classifier

Run `grace/rulesbook/pr-reviewer/prompts/classifier.md`.

The classifier should include `grace-generated-pr` so the extra code-pattern subagent runs.

## Step 4: Spawn the GRACE code-pattern subagent

Run `grace/rulesbook/pr-reviewer/subagents/grace-generated-pr.md`.

This subagent checks for code-pattern issues common in generated PRs:

- copy-paste remnants
- scope drift
- incomplete wiring
- brittle parsing or serialization
- missing companion code updates

## Step 5: Run the regular scenario subagents

Run the scenario subagents required by the classifier for the actual changed code.

## Step 6: Aggregate

Run `grace/rulesbook/pr-reviewer/prompts/aggregator.md` and keep the final report code-only.

## Portable Invocation Prompt

```text
Review PR <PR-REFERENCE> in connector-service.
This PR was raised by 10xGRACE.
Read grace/rulesbook/pr-reviewer/workflows/grace-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```
