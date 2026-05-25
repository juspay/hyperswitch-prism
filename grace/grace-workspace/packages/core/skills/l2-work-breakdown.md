---
name: l2-work-breakdown
description: Level 2 — decompose the L1 spec into discrete, independently-implementable, independently-testable work items inside the hyperswitch-control-center repo. Make internal dependencies explicit so the work items can run in parallel wherever possible.
applies_to: l2_gen
agents:
  - existing-feature-finder
  - conventions-scout
---

# L2 Skill — Work Breakdown

## Purpose

Take the approved L1 spec and split it into discrete units of work. Each unit
must be:

1. **Independently implementable** — an engineer can start it without waiting
   on another unit to finish.
2. **Independently testable** — the unit has its own acceptance check that
   does not require the rest of the feature to exist.
3. **Explicit about dependencies** — if unit B genuinely cannot start until
   unit A lands, say so and justify it.

The guiding question: **"If I handed each of these units to a different
engineer, could they all work in parallel without blocking each other?"**
If the answer is "no" for most pairs, the breakdown needs refinement.

## Scope — single repo

All units live inside `hyperswitch-control-center`. There are no cross-repo
coordination units. If the L1 spec references a backend change, either:

- assume the backend contract is already agreed (and note it as a
  precondition), OR
- surface it as a blocking **Open question** to kick back to L1.

Do not create work items against other repos.

## Context you receive

- `l1` — the approved L1 spec (what + how).
- `task` — original task definition.
- Any reviewer feedback from the L1 gate.
- `agentReports` — condensed findings from the subagents declared below.
  NEVER read the control-center repo directly from this checkpoint. All
  codebase grounding comes from agent reports, so your context window stays
  clean and the breakdown is based on facts from the real repo.

## Subagents

The pipeline spawns these in parallel before calling the main LLM. Each
agent does focused work outside the main context window and returns a
compact `AgentReport` (`{agent, findings[], citations[], notes?}`). You
MUST ground every work item in at least one agent finding — if an agent
returns nothing relevant, say so and narrow the scope accordingly.

### existing-feature-finder

- **Job:** Greps `src/screens/**`, `src/components/**`, and the router for
  keywords extracted from the L1 spec and task title. Identifies anything
  already in the repo that overlaps with the requested feature so L2 can
  decide reuse vs. new module.
- **Input:** task title, task description, L1 `affected areas` and `data
  flow` sections.
- **Returns:**
  - `findings` — one bullet per matching surface, each shaped as
    `"<file-or-module> — <what it does> — <overlap with this task>"`.
  - `citations` — list of file paths (no contents).
  - `notes` — optional one-liner if nothing matched and why.
- **Budget:** ≤ 20 findings, ≤ 1500 tokens total.

### conventions-scout

- **Job:** Reads a small fixed set of anchor files (screens index, router,
  a representative screen folder, a representative `useQuery` hook) and
  extracts naming conventions, folder layout rules, and state-management
  patterns. The goal is for L2 to say "new module under
  `src/screens/<area>/<SubArea>`" using the same shape as existing code.
- **Input:** L1 `affected areas` list (to pick which anchor area to sample).
- **Returns:**
  - `findings` — bullets like
    `"screens folder uses PascalCase directory names with a single .res entry file"`,
    `"query hooks live in src/api/<domain>/use<Domain>.res and return Belt.Result"`.
  - `citations` — the anchor files it read.
- **Budget:** ≤ 12 findings, ≤ 1000 tokens total.

## What you must produce

A list of work items. For each:

### Work item format

- **id** — short slug, e.g. `wi-01-settings-form`.
- **title** — 4–8 words, action-oriented ("Add currency selector to
  business-profile form").
- **area** — the dashboard surface this lives in (Settings, Payments,
  Connectors, etc.).
- **description** — 2–4 sentences explaining what this unit delivers on its
  own.
- **deliverable** — the concrete artifact at the end of this unit (e.g. "new
  currency dropdown renders and persists to merchant account via existing
  update API").
- **parallelism group** — an integer. Items sharing a group can start at the
  same time. Item in group N may assume all items in groups < N are merged.
- **depends on** — list of `id`s this unit waits for. Usually empty.
- **test plan** — how you verify this unit in isolation (unit test, component
  test, manual smoke check). Must not require other units.
- **risk** — low / medium / high, with a one-line reason.

## Rules

- PREFER many small items in parallelism group 0. The best breakdown has most
  items starting at the same time.
- NEVER create a unit whose test plan is "once the whole feature lands, check
  X". That is a sign the unit is not really independent — split or merge it.
- NEVER cross into technical design (data structures, function signatures,
  ReScript types). That is L3's job. Stay at "what does this unit deliver"
  granularity.
- NEVER invent code references. Use the module/surface names from the L1
  spec; if you need a new module, say "new module under `src/screens/…`" and
  leave the path generic.
- If a unit has more than 2 dependencies, it is probably too large — split it.
- If two units always change the same file, they are not parallel — merge
  them or stagger them explicitly.

## Output schema

The pipeline expects a JSON object:

```json
{
  "workItems": [
    {
      "id": "wi-01-…",
      "title": "…",
      "area": "…",
      "description": "…",
      "deliverable": "…",
      "parallelismGroup": 0,
      "dependsOn": [],
      "testPlan": "…",
      "risk": "low"
    }
  ],
  "preconditions": ["… any backend/API assumption this breakdown relies on …"],
  "openQuestions": ["… anything that blocks L3 …"]
}
```

## Quality checklist

- [ ] At least 60% of work items are in parallelism group 0.
- [ ] No item depends on more than 2 others.
- [ ] Every item has a standalone test plan.
- [ ] No item dives into function signatures or ReScript types.
- [ ] Preconditions list any backend contract this plan assumes.
- [ ] Open questions list any ambiguity that would stall L3.
