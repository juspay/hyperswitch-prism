# Methodology — coverage-report

## What the data is

`data/field_probe/*.json` (one file per connector) is produced by a **static, no-network** probe
(`crates/internal/field-probe`, `make field-probe`) that, for every `connector × flow × payment-method`,
calls the connector's request transformer in-process and records a `status`. The files are **git-tracked**,
so any past state is reconstructable with `git show <rev>:data/field_probe/<c>.json` — no re-probe needed.

`scripts/generators/docs/coverage_summary.py` aggregates these. Reuse its `compute()`,
`load_probe_files()`, and `normalize_status()` rather than re-implementing.

## Units

- **PM coverage** — unit = `connector × payment-method` on the **Authorize** flow (the only PM-aware flow).
- **API coverage** — unit = `connector × flow`; a flow counts `supported` if **any** payment method on it is
  supported, else `not_implemented` if any normalizes there, else excluded.
- **Connectors** — count of probe files (one per connector).

## Status model (after normalization)

Raw probe statuses are `supported`, `not_implemented`, `not_supported`, `error`. `normalize_status()`
collapses them to the reported model:

| Raw | Reported | Meaning |
|---|---|---|
| `supported` | supported | transformer produced a valid request — capability exists |
| `not_implemented` | not_implemented | we haven't built it yet (the actionable gap) |
| `error` | → not_implemented | unfinished integration (legacy bucket; newer probe emits 0 of these) |
| `not_supported` | **excluded** | connector genuinely doesn't offer it — out of scope, not a gap |

Coverage share = `supported / (supported + not_implemented)`. `not_supported` is deliberately **not** in the
denominator.

## The trust rule (why this skill exists)

**Quote `supported` COUNTS as growth. Treat shares/percentages as suspect until the taxonomy-shift check passes.**

`supported` counts only rise when real capability is added, and are **unaffected** by reclassification
between `not_implemented` and `not_supported`, or by removing the `error` bucket. Shares are not: they move
whenever the *denominator* changes for non-work reasons.

### The contamination that motivated this (2026-05 → 2026-06)
Two probe-classification changes landed alongside real work that month:
1. **PR #1164 "add messages for unsupported flows"** (+ cryptopay): connectors began explicitly declaring
   unsupported flows, so **+658** `connector × flow/method` combos moved into `not_supported` (excluded).
   That shrank the API denominator and pushed the **API supported share 25% → 33%** — almost entirely a
   measurement artifact, not shipped work.
2. The **`error` bucket was removed** (624 → 0), folded into `not_implemented`.

Through all of that, supported counts told the truth: connectors 86→90, API supported 670→711 (+41),
PM supported 487→514 (+27) — ~6% real capability growth.

### Drift — PM, API, and total
"Drift" = **measurement reclassification between the two snapshots**, not shipped work. Cookbook §1 reports it
per view so you can quantify it:
- **PM drift** — over `connector × method` (Authorize) units: `|Δnot_supported| + |Δerror|`, as a count and
  as % of base PM units.
- **API drift** — over `connector × flow` units (one raw flow-level label per flow, priority
  `supported > not_implemented > error > not_supported`): same formula.
- **TOTAL drift = PM drift + API drift** (count). **TOTAL drift % = average of the two per-view rates =
  (PM% + API%) / 2** (e.g. `(2.5% + 24.3%) / 2 = 13.4%`) — weights the PM and API views equally regardless of
  their unit counts. The per-view %s stay informative (API's per-view % is the real share-contamination signal).

Why those buckets: `not_supported` is excluded from the denominator and `error` was a transitional bucket, so
movement in them changes the *measurement base*, not capability. `supported` growth is unaffected by drift —
that's why it's the metric to quote.

**Gate:** if **TOTAL drift ≥ 50**, the share/% is an apples-to-oranges comparison → present supported counts
only, and state the drift figure so the audience knows the percentage moved for measurement reasons.

This is conservative and count-based (it flags *that* the base moved, not *why*) — e.g., adding several new
connectors also injects `not_supported` entries and can trip the gate without a true reclassification. A fired
gate is a prompt to read the per-view raw table §1 prints, not proof of contamination.

## Quote this / never quote that

| ✅ Quote | ❌ Never quote as growth |
|---|---|
| Supported COUNT deltas (connectors, API flows, PM methods), absolute + relative % | A share/% jump when a taxonomy shift fired |
| New connectors added; per-connector "top movers" | "not_implemented dropped" without noting where it went (often → not_supported) |
| Merged-PR counts/categories driving the change | A single headline % with no caveat |

## Which "main" — origin vs prism (read from git, don't checkout)

This clone has two diverging `main`s:
- **`origin/main` = juspay/connector-service** — what local `main` tracks (the public upstream).
- **`prism/main` = juspay/hyperswitch-prism** — where the team's PRs merge; **ahead** of origin (e.g. on
  2026-06-08, +19 field_probe files / +2949 lines).

Coverage MUST be measured on **`prism/main`** so it matches PR activity (§2 also queries
`juspay/hyperswitch-prism`). Because `data/field_probe/*.json` is git-tracked, the skill reads `prism/main`
**directly from git** (`git show prism/main:…`) after `git fetch prism main` — no stash, checkout, or pull.
That's safer (the working tree is never mutated, so nothing can be stranded) and avoids the trap where
`git checkout main && git pull` would silently report **origin** (connector-service) data instead.

## Baseline selection

The repo tags a daily CalVer release `YYYY.MM.DD.N` (weekdays only) at each `chore(version)` commit on
`main`. Recommended baselines:
- **Month boundary:** a date string → `git rev-list -1 --before=<YYYY-MM-DD> main`. Preferred — robust to
  weekend gaps where no tag exists.
- **Since last release:** the CalVer tag → `git rev-parse <tag>` (or
  `git describe --tags --match '????.??.??.*' --abbrev=0 HEAD`).
- **Rolling window:** `--before='N days ago'`.
- **Explicit commit:** pass the SHA.

## PR attribution

Use the **`prism` remote** → `gh pr list --repo juspay/hyperswitch-prism`. (`origin` =
`juspay/connector-service` has 0 PRs — wrong repo.) Categorize primarily by conventional-commit title prefix
(`feat(connector)` / `fix(connector)` are the strongest, most reliably present signal); labels
(`integration`, `PMT`, `API-FLOW`, `GRACE`) are secondary because only ~50% of PRs are labeled. Exclude
`chore(version)` release-bump PRs from feature counts.
