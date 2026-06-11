---
name: coverage-report
description: >
  Use when generating connector coverage or capability metrics for meetings, manager/stakeholder
  updates, retrospectives, or "what changed" / month-over-month / since-release comparisons in the
  hyperswitch-prism (UCS) repo. Covers coverage diffs over a time window, current-state snapshots,
  merged-PR activity summaries, and per-connector / per-flow drilldowns. Triggers: "data for the
  meeting", "how much did coverage change last month", "what did we ship", "supported vs last release".
license: Apache-2.0
compatibility: >
  Requires Python 3.9+, git, and (for PR activity only) the gh CLI authenticated against github.com.
  Reuses scripts/generators/docs/coverage_summary.py and the field-probe data; run from repo root.
metadata:
  author: tushar.shukla
  version: "1.0"
  domain: reporting
---

# Coverage Report

## Overview

Produces meeting-ready connector-coverage and capability metrics for the hyperswitch-prism repo by
reusing the existing field-probe data (`data/field_probe/*.json`, git-tracked) and the aggregation in
`scripts/generators/docs/coverage_summary.py`. Because the probe data is committed, any historical
baseline is reconstructable from git — no re-probe needed.

**Core principle: quote `supported` COUNTS as the growth signal.** Coverage *percentages/shares* can lie:
when the probe's status taxonomy shifts between two snapshots (a status bucket added/removed, or many
combos reclassified to `not_supported` which is excluded), the share moves for non-work reasons. Always
run the taxonomy-shift check and state its caveats before presenting any percentage.

## When to use

- A manager/stakeholder asks for coverage data, "what changed", or a month-over-month / since-release delta.
- You need current coverage numbers, a merged-PR activity summary, or which connectors/flows moved.
- NOT for: deep code review, generating the full `all_connector.md` docs (that's `make docs`), or
  non-coverage metrics.

## Modes — pick the one that answers the question

Run the exact commands from `references/command-cookbook.md` (each section is copy-paste, read-only, prints
inline). For metric definitions and the caveat rules, read `references/methodology.md`.

| Mode | Answers | Cookbook section | Needs `gh`? |
|---|---|---|---|
| **Diff** (flagship) | "How much did coverage change since X?" supported deltas + **PM/API drift** + caveats | §1 Diff | no |
| **PR activity** | "What did we ship in the window?" merged PRs bucketed by type/label | §2 PR activity | yes |
| **Snapshot** | "Where do we stand now?" current PM/API supported + shares | §3 Snapshot | no |
| **Drilldown** | "Which connectors/flows moved / are new?" top movers | §4 Drilldown | no |

For a full "meeting packet", run Diff + PR activity together (the diff gives the numbers, PR activity gives
the "why").

## Freshness — "latest main" (default data source)

By default the skill reports coverage **as of latest main, read straight from git** — it never stashes,
checks out, or pulls, so your working tree / branch / stash stay untouched.

- **`BASE=prism/main`** is the default base ref. **`prism` = juspay/hyperswitch-prism** — the remote your PRs
  merge into. (Your local `main` tracks `origin` = juspay/connector-service, a *different, diverging* repo —
  don't use it for these reports.)
- Run cookbook **§0** first: `git fetch prism main` — the only network step, offline-tolerant (falls back to
  the cached ref and warns).
- **Same-remote rule:** the coverage base (`$BASE`) and the PR-activity repo (§2) MUST be the same remote
  (`prism`), or the two halves describe different worlds.
- Set **`SOURCE=worktree`** to instead compare your local checkout ("what does my branch add over main").

## Baseline selection (`SINCE`)

The Diff/Drilldown modes need a baseline, resolved on the **`$BASE`** lineage (default `prism/main`):
- **date** `YYYY-MM-DD` (default: first of the current month) → `git rev-list -1 --before=<date> $BASE`
  (robust to weekend gaps — preferred for month boundaries).
- **CalVer tag** `YYYY.MM.DD.N` → `git rev-parse <tag>`.
- **N-days** `30d` → `git rev-list -1 --before='30 days ago' $BASE`.
- **commit SHA** → used directly.

## Methodology rules (HARD — do not skip)

- **Report against latest `prism/main`, read from git** (cookbook §0 `git fetch prism main`). Never
  stash/checkout/pull — the working tree is left untouched. Coverage base and PR repo must BOTH be `prism`.
- **Quote supported COUNTS as growth.** They're immune to the not_implemented↔not_supported boundary shift.
- **Always report drift** — cookbook §1 prints **PM drift / API drift / TOTAL drift** (status
  reclassification: `not_supported`/`error` churn, NOT shipped work). If **TOTAL drift ≥ 50**, **do NOT
  present share/% as growth** — quote supported counts and say why.
- `not_supported` is **excluded** from coverage; `error` folds into `not_implemented` (matches
  `coverage_summary.py`). Apply the same aggregation to both snapshots (the cookbook does).
- PR queries use the **`prism` remote → `juspay/hyperswitch-prism`**, NOT `origin` (`juspay/connector-service`
  has 0 PRs). Exclude `chore(version)` release-bump PRs from feature counts.

## Presentation (default: inline)

Present the **Coverage Growth Summary** markdown table that §1 prints — columns `Metric | Base | Current |
Change | Growth`, rows Connectors / API Supported (Flows) / PM Supported (Methods). The API/PM Supported
cells show `supported/total (pct%)` where total = supported + not_implemented. Follow with the **Highlights**
bullets and the **Overall** line (§1 prints these too). **Always keep the one-line footnote** that the API/PM
`%` is denominator-sensitive (its base shrinks as flows are reclassified to `not_supported`), so the
trustworthy growth signal is the **counts / Change**, not the `%`. Only write a markdown file, Slack blurb, or
CSV if the user explicitly asks.

## Common mistakes

| Mistake | Fix |
|---|---|
| Quoting an API "share" jump (e.g. 25%→33%) as growth | It's usually a `not_supported` reclassification shrinking the denominator. Quote supported counts. |
| Querying `juspay/connector-service` (`origin`) | Use `juspay/hyperswitch-prism` (`prism`). |
| Counting `chore(version)` PRs as features | Exclude them. |
| Assuming a weekend CalVer tag exists | Use the date form (`--before=<date>`), which handles gaps. |
| Comparing snapshots with different script versions | Always run the *current* `coverage_summary.py` against both. |

## References
- `references/command-cookbook.md` — exact copy-paste commands for every mode.
- `references/methodology.md` — metric definitions, the taxonomy-shift contamination story, what to quote.
