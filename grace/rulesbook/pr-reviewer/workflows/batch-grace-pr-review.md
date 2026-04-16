# Batch GRACE PR Review Workflow

Use this workflow to review all currently open PRs raised by `10xGRACE`.

## Goal

Fan out one PR review job per open GRACE PR, and inside each PR review, fan out one scenario subagent per matched scenario.

This is the highest-parallelism workflow in the system:

- outer fan-out: one review run per PR
- inner fan-out: one scenario subagent per matched scenario within that PR

## Step 0: Load the review system

Read these first:

- `grace/rulesbook/pr-reviewer/README.md`
- `grace/rulesbook/pr-reviewer/workflows/grace-pr-review.md`
- `grace/rulesbook/pr-reviewer/config/path-rules.yaml`
- `grace/rulesbook/pr-reviewer/config/rubric.yaml`

## Step 1: List open GRACE PRs

Use GitHub metadata to find open PRs where at least one of these is true:

- PR author is `10xGRACE`
- head repo owner is `10xGRACE`

## Step 2: Spawn one PR review per PR

For each matched PR, run:

```text
Review PR <PR-REFERENCE> in connector-service.
This PR was raised by 10xGRACE / GRACE automation.
Read grace/rulesbook/pr-reviewer/workflows/grace-pr-review.md and follow it exactly.
Use nested subagents if the tool supports them; otherwise emulate the same phases serially.
```

## Step 3: Aggregate batch results

For the batch summary, report per PR:

- PR number and title
- primary scenario
- final decision
- top blocking reason if any

## Success Criteria

The batch is complete only when each open `10xGRACE` PR has its own review result.
