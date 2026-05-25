---
name: l3-technical-approach
description: Level 3 — for each L2 work item, define the technical approach, data structures, API contracts, integration points, and test expectations inside hyperswitch-control-center. Output must give an engineer or coding agent enough detail to start implementation without further clarification.
applies_to: l3_gen
agents:
  - api-explorer
  - types-explorer
  - test-explorer
---

# L3 Skill — Technical Approach and Subtasks

## Purpose

Take each work item from L2 and make it implementable. At this level you
define the specific design decisions: data structures, API contracts,
component boundaries, state ownership, and integration points. You also
define test expectations — what existing tests might break, what new tests
are required, and what acceptance criteria look like.

After L3, an engineer (or an AI coding agent) should be able to start typing
code without coming back to ask "how should this work?".

## Scope — single repo

All decisions apply to `hyperswitch-control-center`. The repo is ReScript +
React with the existing conventions:

- Screens live under `src/screens/**`.
- Shared components under `src/components/**` or domain-specific folders.
- API hooks typically under `src/api/**` or `src/queryClient/**` (verify
  against the actual repo before committing a path).
- Routing is file-based under the screens folder; new routes must be wired
  through the screen entry file.
- Types are expressed as ReScript types and records — not TypeScript, not
  plain JS.

If an existing pattern fits, use it. If a new pattern is needed, justify it
explicitly.

## Context you receive

- `l1` — approved product spec.
- `l2` — approved work breakdown with work items.
- `task` — original task definition.
- Any reviewer feedback from the L2 gate.
- `agentReports` — condensed findings from the subagents declared below.
  NEVER read the control-center repo directly. Every type name, hook, file
  path, or test file you reference must come from an agent report.

## Subagents

The pipeline spawns these in parallel before calling the main LLM. Each
returns an `AgentReport` (`{agent, findings[], citations[], notes?}`).

### api-explorer

- **Job:** Scans `src/api/**`, query client hooks, and any `useXxxQuery`/
  `useXxxMutation` entries that match L2 work-item keywords. Identifies
  which backend endpoints are already wired and which would need new
  wiring.
- **Input:** L2 `workItems[]` (titles + descriptions), L1 API-surface
  section.
- **Returns:**
  - `findings` — bullets shaped as
    `"useMerchantAccount — GET /account/:id — returns merchantAccount record — reusable for wi-01"`.
  - `citations` — file paths of the hooks/modules found.
  - `notes` — list any work item that needs a brand-new endpoint and
    whether that is a precondition from L1.
- **Budget:** ≤ 25 findings, ≤ 2000 tokens.

### types-explorer

- **Job:** Greps `.res` files under `src/**` for record and variant
  declarations matching the work-item nouns (e.g. "profile", "currency",
  "connector"). Identifies reusable types and any fields close to what L3
  needs.
- **Input:** L2 work items + nouns extracted from the L1 spec.
- **Returns:**
  - `findings` — bullets like
    `"type merchantProfile = { id, displayName, currency, ... } — src/types/MerchantTypes.res — reusable for wi-01, extend with localeTag"`.
  - `citations` — file paths.
  - `notes` — any noun that has no matching type and will need a new one.
- **Budget:** ≤ 20 findings, ≤ 1500 tokens.

### test-explorer

- **Job:** Scans test folders (`tests/**`, `**/__tests__/**`,
  `**/*.test.*`, `**/*.spec.*`) for files that overlap with the work
  items' target screens/components. Flags tests likely to break.
- **Input:** L2 work items + L1 `affected areas`.
- **Returns:**
  - `findings` — bullets shaped as
    `"BusinessProfileForm.test.res — asserts currency dropdown absent — will break under wi-01, needs updated fixture"`.
  - `citations` — test file paths.
  - `notes` — any surface that has NO tests today (worth flagging as a
    new-test requirement in L3).
- **Budget:** ≤ 15 findings, ≤ 1200 tokens.

## What you must produce

For each work item from L2, produce a **technical approach** entry:

### Entry shape

- **workItemId** — must match an `id` from L2.
- **approach** — 3–6 sentence narrative: what you are going to change, at
  what layer (screen / component / hook / API), and why this is the smallest
  change that satisfies the deliverable.
- **data model**
  - ReScript types/records you will add or extend (name + field list, no
    raw syntax yet — that is L4).
  - Where the state lives (component local state, context, query cache,
    URL, localStorage).
- **API contract**
  - For each backend call: endpoint path, method, request fields, response
    fields, error cases the UI must handle.
  - If the endpoint already exists, reference the existing hook/function
    name. If new, mark as "requires backend support — see L1 precondition".
- **integration points**
  - Which existing screens/components are touched.
  - Which routes are added or modified.
  - Which shared utilities (date formatting, currency, feature-flag helpers)
    are reused.
- **subtasks** — ordered list of 2–6 bullet-sized steps inside this work
  item. Each subtask is about ½–2 hours of work. Order matters.
- **test expectations**
  - New unit/component tests to add, with a one-line description of what each
    asserts.
  - Existing tests likely affected — name the test file or describe it so a
    reviewer can find it.
  - Manual verification steps for behavior that cannot be covered by
    automated tests (visual polish, keyboard nav, etc.).
- **acceptance criteria** — checklist form. Each item must be objectively
  verifiable (a reviewer can answer yes/no without judgment).
- **risks and fallbacks** — what is most likely to go wrong and the escape
  hatch if it does.

## Rules

- ANCHOR every file path, hook name, and component name in patterns that
  actually exist in the control-center repo. If unsure, mark with
  "(confirm path)" and explain what you expect to find.
- DO NOT write ReScript source code here. Type names and field lists are
  fine. Full type/let definitions belong in L4.
- DO NOT invent backend endpoints. If you need a new endpoint, say so and
  mark the work item as depending on an L1 precondition.
- DO NOT skip test expectations. A work item with no tests is not L3-ready.
- **STRONGLY PREFER reusing and extending existing screens, components,
  hooks, and utilities over creating new ones.** This is the single biggest
  source of downstream bugs. When an L2 work item adds behavior to an
  existing screen, the default L3 answer is: extend the existing screen
  file, extend the existing hook, extend the existing type. Creating a new
  `*Form.res`, `*Hooks.res`, `*Utils.res`, or `*Types.res` file next to an
  existing screen is almost always wrong. Call out the reuse explicitly
  ("extends existing `useMerchantAccount`", "adds field to existing
  `ProductionIntentForm.res`"). If you propose a new file, state in the
  `approach` narrative which existing file you considered and the concrete
  reason it did not fit.
- If two work items share a data type, define it once and reference it from
  both — avoid duplication that L4 will have to reconcile.

## Output schema

```json
{
  "entries": [
    {
      "workItemId": "wi-01-…",
      "approach": "…",
      "dataModel": {
        "types": [{ "name": "…", "fields": ["…"] }],
        "stateLocation": "…"
      },
      "apiContract": [
        {
          "endpoint": "/api/…",
          "method": "POST",
          "request": ["…"],
          "response": ["…"],
          "errorCases": ["…"],
          "existingHook": "useX | null"
        }
      ],
      "integrationPoints": {
        "screens": ["…"],
        "components": ["…"],
        "routes": ["…"],
        "sharedUtils": ["…"]
      },
      "subtasks": ["…"],
      "testExpectations": {
        "newTests": [{ "file": "…", "asserts": "…" }],
        "affectedTests": ["…"],
        "manualVerification": ["…"]
      },
      "acceptanceCriteria": ["…"],
      "risksAndFallbacks": "…"
    }
  ]
}
```

## Quality checklist

- [ ] Every L2 work item has a corresponding L3 entry.
- [ ] Every entry names real (or explicitly flagged) file paths.
- [ ] Every entry has at least one automated test expectation.
- [ ] Acceptance criteria are objective (no "looks good" or "feels snappy").
- [ ] No ReScript source bodies — only type/field names and narrative.
- [ ] Reuse of existing hooks/components/utilities is explicit.
