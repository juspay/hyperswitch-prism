# hs-prism-learn-mcp

An MCP server that helps someone **new to the hyperswitch-prism (UCS) codebase** understand how it works — accurately, and in plain language. It is the wayfinding + explainer layer over the repo's scattered docs, skills, and code.

It is the sibling of [`integration-mcp`](../integration-mcp): that one helps **external developers embed the Prism SDK into their app**; this one helps **people understand the repo itself** (contributors, reviewers, and the curious — including non-engineers).

## Why it exists

The knowledge a newcomer needs is real but scattered across 160+ docs, 8 `.skills/` playbooks, the `grace/` codegen rulesbook, 95 Rust connectors, and `payment.proto`. Worse, some docs disagree with each other (e.g. Refund's prerequisites). This server gives a newcomer one place to ask:

- "What is this project?" → `prism_learn_start_here`, `prism_learn_architecture_overview`
- "What is a connector / flow / transformer / RouterDataV2 / grace?" → `prism_learn_explain_concept`
- "Where does X live?" → `prism_learn_repo_map`, `prism_learn_search`
- "How do I add a connector / flow / payment method?" → `prism_learn_how_to`
- "Explain the Authorize flow / walk me through Stripe." → `prism_learn_explain_flow`, `prism_learn_connector_walkthrough`
- "Why won't my connector compile?" → `prism_learn_troubleshoot`
- "Show me the source." → `prism_learn_read_doc`

**Every answer cites a real file path.** It never invents facts about the repo, and when it doesn't know, it says so.

## How it stays accurate (no hallucination)

Like `integration-mcp` codegens connector data from `payment.proto`, this server is built from the repo itself:

- **A build-time indexer** (`scripts/gen-knowledge.ts`) walks the repo's real docs/skills/grace/connector-registry and emits committed JSON (`src/data/*.generated.json`). The published package ships that JSON, so it works via `npx` outside a checkout.
- **A small hand-authored layer** (`content/`): plain-language concept cards, learning paths, a repo map, and a known-discrepancies registry. This is the teaching the raw docs lack.
- **A drift gate** (`scripts/validate-content.ts`, run in `prebuild` and CI): the build **fails** if any cited path or anchor substring has disappeared, or any concept/path cross-link breaks. That is the guarantee that the curated prose stays true to the code.

Volatile facts (connector counts, coverage, status codes) are read from the repo and labeled by source — never hardcoded. Where docs disagree, the server surfaces the discrepancy instead of silently picking.

## Tools

| Tool | What it answers |
|---|---|
| `prism_learn_start_here` | "I'm new — where do I begin?" Orientation + a path for your role. |
| `prism_learn_architecture_overview` | The big picture, end to end (`depth`: tldr / standard / deep). |
| `prism_learn_explain_concept` | "What is X?" Plain-language concept cards with analogies + citations. |
| `prism_learn_glossary` | Look up a term (verbatim from the repo glossary). |
| `prism_learn_repo_map` | "Where does X live?" Topic/symbol → verified path. |
| `prism_learn_search` | Keyword search across docs, skills, and grace guides. |
| `prism_learn_read_doc` | Read any indexed doc (or one section) verbatim. |
| `prism_learn_how_to` | Route a task to the right `.skills/` playbook. |
| `prism_learn_explain_flow` | Explain a flow, its dependencies, and support. |
| `prism_learn_connector_walkthrough` | Tour a real connector (default `stripe`). |
| `prism_learn_learning_path` | A step-by-step curriculum for your role. |
| `prism_learn_troubleshoot` | Symptom → cause → fix, with citations. |
| `prism_learn_coverage` | Which connectors support which flows. |
| `prism_learn_faq` | Answers from `docs/FAQs.md`. |

Resources: `prism://learn/{start,architecture,glossary,repo-map,skills-index,payments-101,llms-txt}`.

## Use it

```jsonc
// Claude Code / Cursor / Windsurf mcp config (runs alongside integration-mcp)
{
  "mcpServers": {
    "hs-prism-learn": { "command": "npx", "args": ["-y", "hs-prism-learn-mcp"] }
  }
}
```

Or directly:

```bash
npx hs-prism-learn-mcp            # stdio
npx hs-prism-learn-mcp --http --port 3000
```

## Develop

```bash
npm install
npm run gen:knowledge   # re-index the repo into src/data/*.generated.json
npm run validate        # run the drift gate (dead paths / anchors / slugs)
npm run build           # prebuild (gen + validate) then tsc + copy JSON
npm run smoke           # spawn the built server and exercise every tool
npm run inspect         # open the MCP Inspector
```

The indexer finds the repo via `PRISM_REPO_ROOT`, or by walking up from the package (it expects to sit next to `docs/`, `.skills/`, and `crates/`). Editing a concept card? Keep its `go_deeper` paths and `verify_anchors` pointing at real files — the drift gate will block the build otherwise.

License: Apache-2.0.
