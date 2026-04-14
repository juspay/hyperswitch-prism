# Incremental Review Workflow

Use this when a PR has already been reviewed once and you need to verify the latest author updates.

## Goal

Re-check the updated diff without losing the strictness of the full review.

## Step 0: Load the review system

Read these first:

- `grace/rulesbook/pr-reviewer/README.md`
- `grace/rulesbook/pr-reviewer/config/path-rules.yaml`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`
- `grace/rulesbook/pr-reviewer/config/output-template.md`

## Step 1: Gather delta context

Collect:

- files changed since the last review round
- prior blocking findings and warnings
- PR author and head repo owner when needed for GRACE scenario activation

## Step 2: Re-classify if scope changed

Run `grace/rulesbook/pr-reviewer/prompts/classifier.md` again if:

- new files were added
- scenario mix changed
- new high-risk paths appeared

If scope did not change materially, keep the prior classification but confirm it still fits.

## Step 3: Re-run only the necessary reviewer families

Re-run any scenario subagent whose area changed or whose previous blocker depended on updated evidence.

Minimum rule:

- always re-run the reviewer that raised a blocker if its relevant files changed
- always re-run the scenario subagent that raised a blocker if its relevant files changed
- re-run `ci-config-security` if workflows, config, credentials, or verification logic files changed
- re-run `tests-docs` if tests, specs, generated docs, or source-of-truth companions changed
- re-run `grace-generated-pr` if the PR still routes through the `10xGRACE` code-pattern lens and relevant code changed

## Step 4: Aggregate again

Run `grace/rulesbook/pr-reviewer/prompts/aggregator.md`.

The final report should explicitly say:

- which prior blockers are fixed
- which blockers remain
- whether new blockers were introduced

## Portable Invocation Prompt

```text
Re-review PR <PR-REFERENCE> in connector-service after the latest updates.
Read grace/rulesbook/pr-reviewer/workflows/incremental-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```
