---
name: l1-what-and-how
description: Level 1 — produce a product-level specification that answers WHAT is being built and HOW it fits into the existing hyperswitch-control-center codebase. Covers user-facing behavior, expected outcomes, edge cases, affected modules, data flow, and API surface changes.
applies_to: product_alignment
---

# L1 Skill — What and How

## Purpose

Turn a raw task description into a crisp product-level specification. At this
level the audience is a product manager + senior engineer deciding whether to
invest in the work, not an implementer. You must answer two questions plainly:

1. **What is the feature or change?** User-facing behavior, expected outcomes,
   edge cases.
2. **How does it fit into the existing system?** Which modules are affected,
   what data flows change, what APIs are introduced or modified.

## Scope — single repo

The target repo is `hyperswitch-control-center` (ReScript + React dashboard).
All references, file paths, and module names must be grounded in that repo.
There is no cross-repo work at this level — do NOT invent references to
`hyperswitch`, `hyperswitch-web`, or any other repo.

## Context you receive

- `task` — raw task definition (title, description, acceptance criteria,
  optional figmaUrl).
- Any prior clarifications from the product alignment loop.

## What you must produce

A structured L1 spec with these sections:

### 1. What
- Plain-language description of the user-visible change (2–4 sentences).
- Primary user story ("As a merchant, I want … so that …").
- Expected outcome — what success looks like from the user's perspective.
- Edge cases the product expects handled (empty states, errors, permission
  variants, feature flags, etc.).

### 2. How it fits
- **Affected areas** — list the dashboard surfaces touched (e.g. Settings →
  Business Profile, Payments list, Connectors page). Use real navigation labels
  from the control-center, not invented ones.
- **Data flow** — where does the data originate (backend API, local state,
  URL param), how does it move through the dashboard, where does it end up.
  Call out any new API calls.
- **API surface** — list any new or modified control-center ↔ backend API
  interactions. For each: endpoint path, method, request/response shape at a
  high level (field names, not byte-level types).
- **Config / feature flags** — whether this needs a merchant config toggle,
  a feature flag, or a gradual rollout.

### 3. Out of scope
- Explicit list of things this spec does NOT cover, to kill scope creep early.

### 4. Open questions
- Anything that would block the L2 work breakdown. Prefer asking now over
  guessing. Each question should have an owner (product, design, backend).

## Rules

- GROUND every module/file/component reference in the actual control-center
  codebase. If you are not sure a file exists, mark it as "to confirm".
- NEVER include multi-repo language. No references to the Rust backend repos,
  the SDK web repo, or recon/prism/card-vault. This level only describes how
  things look from the dashboard side.
- NEVER go below the module level. No function signatures, no ReScript type
  definitions, no file diffs. That is L3/L4 territory.
- WRITE for a PM + engineer pair, not for an LLM implementer. Short sentences,
  concrete nouns, no hand-waving adjectives ("robust", "seamless", "elegant").
- If the task description is ambiguous, surface it as an **Open question** —
  do not silently fill gaps.

## Quality checklist

Before returning, verify:

- [ ] Every "What" bullet is user-observable (no internal plumbing).
- [ ] Every "How" bullet names a real dashboard surface.
- [ ] Every API mentioned has a path, method, and purpose.
- [ ] Edge cases include at least: empty data, permission denied, network
      failure, and any feature-flag / rollout variant.
- [ ] Out-of-scope list is non-empty (there is always something you are NOT
      doing).
- [ ] No function signatures, no file diffs, no ReScript type blocks.
