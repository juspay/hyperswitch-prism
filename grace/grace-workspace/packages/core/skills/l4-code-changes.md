---
name: l4-code-changes
description: Level 4 — enumerate the concrete code changes required inside hyperswitch-control-center. Lists every file touched, every function added/changed/removed, exact function signatures in ReScript, and the key logic paths. Output is a blueprint an AI coding agent can implement with near-zero ambiguity.
applies_to: l4_gen
agents:
  - file-locator
  - signature-extractor
  - import-scout
---

# L4 Skill — Code Changes Required

## Purpose

Turn the L3 technical approach into a line-by-line blueprint. At this level a
coding agent should be able to start writing ReScript without asking any
further questions. If you find yourself leaving decisions open, the L4 spec
is incomplete — go back and make them.

## Scope — single repo

All changes are inside `hyperswitch-control-center`. ReScript + React is the
only stack in play. Do not reference TypeScript syntax, Rust types, or files
outside this repo.

## Context you receive

- `l1` — product spec.
- `l2` — work breakdown.
- `l3` — technical approach per work item.
- `task` — original task definition.
- Any reviewer feedback from the L3 gate.
- `agentReports` — condensed findings from the subagents declared below.
  NEVER read the control-center repo directly. Every real path, existing
  signature, and import you reference must come from an agent report.

## Subagents

The pipeline spawns these in parallel before calling the main LLM. Each
returns an `AgentReport` (`{agent, findings[], citations[], notes?}`).

### file-locator

- **Job:** For every screen/component/hook/type referenced by L3, resolves
  it to an actual repo path. Confirms whether the file exists today (to
  modify) or needs to be created (to create). Catches hallucinated paths
  before they reach L4's output.
- **Input:** L3 `integrationPoints.*` and any file/module names mentioned
  across L3 entries.
- **Returns:**
  - `findings` — one bullet per L3 reference, shaped as
    `"<L3 reference> → <real path> (exists|missing)"`.
  - `citations` — the real paths it resolved.
  - `notes` — list every L3 reference that could not be resolved; L4 must
    mark those as `create` rather than `modify`.
- **Budget:** ≤ 40 findings, ≤ 2000 tokens.

### signature-extractor

- **Job:** For each file flagged by `file-locator` as `exists` and as a
  modification target, extracts the existing top-level `let` signatures,
  record types, and React component props. Provides the style reference
  so new signatures L4 adds match the local conventions (labelled args,
  `promise<result<_,_>>` vs. `Js.Promise.t`, etc.).
- **Input:** List of existing-file paths from `file-locator`.
- **Returns:**
  - `findings` — bullets shaped as
    `"src/screens/…/Foo.res — let foo = (~id: string) => promise<result<bar, apiError>>"`.
  - `citations` — file paths.
- **Budget:** ≤ 30 findings, ≤ 2000 tokens.

### import-scout

- **Job:** For the same list of existing files, lists the modules each
  file currently imports (`open X`, `module Y = Z`). Gives L4 the right
  module-path style so `importsAdded` entries match how the rest of the
  repo refers to those modules.
- **Input:** List of existing-file paths from `file-locator`.
- **Returns:**
  - `findings` — bullets shaped as
    `"src/screens/…/Foo.res imports: LogicUtils, APIUtils, MerchantTypes"`.
  - `citations` — file paths.
  - `notes` — the single most common import path style observed (e.g.
    `open LogicUtils` vs `module L = LogicUtils`) so new files can follow
    it.
- **Budget:** ≤ 20 findings, ≤ 1200 tokens.

## What you must produce

A per-file change list. Every file the implementation touches must appear
exactly once, grouped under its work item. For each file:

### File entry shape

- **workItemId** — the L2/L3 id this file change belongs to.
- **path** — exact path from the repo root, e.g.
  `src/screens/Settings/BusinessProfile/BusinessProfileForm.res`.
- **action** — one of `create`, `modify`, `delete`, `rename`.
- **purpose** — one sentence: why this file is in the change list.
- **types** — for each ReScript type added or modified:
  - name
  - full type definition (record / variant / alias)
  - whether it is exported
- **functions** — for each function added, changed, or removed:
  - name
  - action (`add` | `modify` | `remove`)
  - signature — full ReScript signature including labelled/optional args and
    return type, e.g.
    `let updateBusinessProfile = (~merchantId: string, ~patch: profilePatch) => promise<result<profile, apiError>>`
  - purpose — one sentence
  - key logic — ordered bullet list of the main steps inside the function.
    Code pseudocode is OK but prefer plain English when the logic is obvious.
- **imports added** — modules newly imported by this file.
- **imports removed** — modules no longer used.
- **wiring** — anywhere this file is referenced from elsewhere that must
  also be updated (e.g. screen registry, route table, parent component's
  props). Link each wiring change to the file that owns it.

### Additional per-work-item sections

- **new routes / navigation** — exact path string + which screen file it
  resolves to.
- **new env / config / feature-flag keys** — name, default value, where
  it is read.
- **migration notes** — if any persisted state shape changes, describe how
  existing values should be interpreted (or discarded, and why that is
  safe).

## Rules

- **REUSE EXISTING FILES. PREFER `modify` OVER `create`.** This is the single
  most important rule. The pipeline's #1 failure mode is emitting `create`
  for a brand-new sibling file (e.g. `ProductionIntentFormHooks.res`,
  `ProductionIntentFormUtils.res`) when the logic belongs inside the
  existing `ProductionIntentForm.res`. DO NOT DO THIS.
  - Default action = `modify`. `create` is the exception.
  - Before emitting any `create`, re-read the `file-locator` findings. If
    ANY existing file plausibly owns this code (same feature, same screen,
    same domain — not just same directory), use `modify` on that file
    instead.
  - Do NOT split one screen into `<Name>.res` + `<Name>Hooks.res` +
    `<Name>Utils.res` + `<Name>Types.res`. Add hooks, utils, and types
    INSIDE the existing screen file unless the screen is already hundreds
    of lines AND the repo demonstrably follows a sibling-file pattern
    (cite the sibling files from `file-locator` findings if so).
  - Every `create` entry's `purpose` field MUST explicitly state: "No
    existing file was suitable because …" and name the existing files
    considered. A `create` without this justification is invalid.
  - When in doubt, `modify`. Creating a new file is almost always wrong.
- EVERY function signature must be complete ReScript, with labels, optional
  markers, and return types. No `...args` and no TypeScript-style
  annotations.
- EVERY file entry must list at least one of: types, functions, wiring,
  imports. An entry with nothing in it is a stale reference — remove it.
- NEVER write full function bodies. Key logic is bullet-form steps, not
  ReScript source. Exception: one-liner helpers (≤ 2 statements) may be
  spelled out verbatim.
- NEVER invent repo paths. If you are guessing at a path, mark it
  "(confirm path)" and describe what you expect to find there.
- NEVER create files that duplicate existing functionality. Before adding a
  new utility, reference the existing one you considered and why it did not
  fit.
- KEEP each function at a single responsibility. If a function's key-logic
  bullet list has more than 6 items, split it and document both halves.
- MATCH the L3 data model exactly — type names, field names, and their
  order should line up. If you diverge, flag it and explain.
- If a file is touched by more than one work item, list it under each work
  item with the subset of changes that belong to that work item. The overall
  file ends up reconstructed from the union.

## Output schema

```json
{
  "changes": [
    {
      "workItemId": "wi-01-…",
      "files": [
        {
          "path": "src/screens/…/Foo.res",
          "action": "modify",
          "purpose": "…",
          "types": [
            {
              "name": "profilePatch",
              "definition": "type profilePatch = { displayName: option<string>, currency: string }",
              "exported": true
            }
          ],
          "functions": [
            {
              "name": "updateBusinessProfile",
              "action": "add",
              "signature": "let updateBusinessProfile: (~merchantId: string, ~patch: profilePatch) => promise<result<profile, apiError>>",
              "purpose": "…",
              "keyLogic": ["…", "…"]
            }
          ],
          "importsAdded": ["…"],
          "importsRemoved": [],
          "wiring": ["… ← update Settings screen registry to surface new route"]
        }
      ],
      "newRoutes": [{ "path": "/settings/business-profile/currency", "screen": "…" }],
      "newConfigKeys": [],
      "migrationNotes": "…"
    }
  ]
}
```

## Quality checklist

- [ ] Every L3 work item has at least one file entry under `changes`.
- [ ] Every file entry has a non-empty body (types, functions, wiring, or
      imports).
- [ ] Every function signature is valid ReScript with labels and return
      type.
- [ ] No function has more than 6 key-logic bullets without being split.
- [ ] Every guessed path is marked "(confirm path)".
- [ ] Type and field names match the L3 data model.
- [ ] Any persisted-state change has migration notes.
- [ ] No references to other repos, no TypeScript, no Rust.
