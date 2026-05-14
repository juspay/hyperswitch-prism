# 10XGRACE

An automated pipeline that turns a frontend task description into a working PR. It walks the task through 16 checkpoints — spec generation, design review, code generation, compile, visual diff, Cypress, Playwright, PR review — and retries from a safe point if anything fails. Built for the **hyperswitch-control-center** (ReScript + React) repo.

You drive the pipeline from the **dashboard UI** at <http://localhost:3141>. The terminal is only for starting things up.

![10XGRACE dashboard](docs/dashboard.png)

> Drop a screenshot of the running dashboard at `docs/dashboard.png` to render it here.

---

## Setup (step by step)

### Step 1 — Check prerequisites

You need all of the following:

- [ ] **Node.js 18+** — verify with `node -v`
- [ ] **pnpm 9** — install with `npm install -g pnpm@9`
- [ ] **opencode CLI** — install from <https://opencode.ai>, verify with `opencode --version`
- [ ] **hyperswitch-control-center** cloned next to this repo so the relative path `../hyperswitch-control-center` resolves
- [ ] An **API key** for an LLM gateway (OpenAI-compatible or Anthropic)

### Step 2 — Install dependencies

From the `10xgrace` repo root:

```bash
pnpm install
pnpm build
```

This builds the three packages: `core`, `cli`, and `dashboard`.

### Step 3 — Edit `config.yml`

All configuration lives in `config.yml` at the repo root. Open it and update only the values below — every other field already has a working default, leave them as they are.

```yaml
projectRoot: ../hyperswitch-control-center        # path to your target repo

llm:
  baseUrl: "https://your-llm-gateway/v1/chat/completions"
  apiKey:  "sk-..."                               # your LLM key
  model:   "kimi-latest"                          # or claude-sonnet-4-6, gpt-4o, etc.
  protocol: openai                                # "anthropic" if your gateway speaks Anthropic
```

That's it for configuration — 10XGRACE picks everything up from this file at startup.

### Step 4 — Start opencode in a separate terminal tab

Open a **second terminal tab** and run:

```bash
opencode serve
```

It listens on `http://127.0.0.1:4096`, which matches `opencode.attachUrl` in `config.yml`. **Keep this tab open** for the whole session — the pipeline calls into it for code-generation steps.

### Step 5 — Start 10XGRACE (engine + dashboard)

Back in your **first terminal**, run:

```bash
pnpm dev
```

This launches the engine, the dashboard server, and rebuilds in watch mode. Once it's up, open <http://localhost:3141> in your browser.

### Step 6 — Create a task in the dashboard

Everything from here is in the UI:

1. Open <http://localhost:3141>.
2. Click **New task** and fill in the title and description.
3. Submit — the pipeline starts automatically.
4. Approve / edit / regenerate at each review gate using the buttons in the dashboard.

You don't need the terminal again unless you want to reset state or stop the engine.

---

## How to update a step

Two ways, depending on what you want:

**Edit a spec at a review gate** (the L2 / L3 / L4 reviews pause and ask you what to do):

- **Approve** — proceed to the next checkpoint.
- **Edit** — open the spec in `$EDITOR` (falls back to nano, then vi). The spec is re-validated when you save. L3 also checks ids, `dependsOn`, and cycles. L4 will ask whether to regenerate code for edited tasks.
- **Regenerate** — type free-text guidance, and the generator runs again with your notes.

**Re-run from an earlier checkpoint** (after fixing something or because a later step failed):

- In the dashboard, click the checkpoint you want to restart from on the run timeline. The engine rewinds and replays from there.

---

## How to remove the database and start fresh

All run history lives in a local SQLite file at `~/.10xgrace/pipeline.sqlite`. To wipe everything and start over:

```bash
pnpm clear
```

This removes the DB, the WAL files, and the resume pointer, then bounces the engine. Refresh the dashboard — the **Past runs** list will be empty.

If you'd rather do it manually:

```bash
rm -rf ~/.10xgrace
```

---

## When things fail

Every checkpoint has a rollback target. After 3 failed retries (default) the run stops.

| If this fails | Pipeline rolls back to |
|---|---|
| task / product_alignment | task |
| design_gate | design_gate |
| l2_* / l3_* / l4_* | the matching `_gen` step |
| implementation / compiler | implementation (with code-repair) |
| design_match | implementation |
| cypress / playwright / pr_review | compiler (with code-repair) |
| regression | cypress |

Tune retry count via `maxRetries` in `config.yml`.

---

## How to add or skip a step

The pipeline runs a fixed list of 17 checkpoints in order. You can **skip** some via config; **adding** a new one is a code change.

### Skip a step (config only)

Right now only the **regression** checkpoint can be skipped from `config.yml`:

```yaml
checkpoints:
  regression:
    enabled: false   # checkpoint runs but returns immediately
```

(`cypress` and `playwright` accept `enabled` in the config type, but their checkpoint code doesn't read it yet — adding the same `if (cfg.enabled === false) return { passed: true };` guard as `regression` is a one-line change in each checkpoint file.)

`design_match` is skipped automatically when the **design_gate** stage decides the task doesn't need a visual design — that's runtime, not config.

### Add a new step (code change)

To insert a new checkpoint, edit three files in `packages/core/src/`:

1. **`types.ts`** — add the new id to the `CheckpointId` union.
2. **`checkpoints/<your-step>.ts`** — export a `Checkpoint` object with `id`, `name`, `description`, `retryFrom` (which checkpoint to roll back to on failure), and an async `run(ctx)` function. Use any existing checkpoint as a template — `regression.ts` is the simplest.
3. **`checkpoints/index.ts`** — import your new checkpoint and add it to the `ALL_CHECKPOINTS` array at the position you want it to run.

Then `pnpm build` and your new step is wired into the pipeline.

> **Heads up:** if your new step belongs in the middle of the pipeline, also update the `retryFrom` of any later step whose rollback should now point at yours.

---

## Project layout

```
10xgrace/
├── config.yml                          # all your settings live here
├── examples/sample-task.json           # example input
└── packages/
    ├── core/
    │   └── src/
    │       ├── types.ts                # CheckpointId union — start here to add a step
    │       ├── checkpoints/
    │       │   ├── index.ts            # ALL_CHECKPOINTS (run order)
    │       │   └── *.ts                # one file per checkpoint
    │       ├── engine.ts               # runs checkpoints, handles retries
    │       └── llm.ts                  # LLM client
    ├── cli/                            # the `10xgrace` command (used by `pnpm dev`)
    └── dashboard/                      # React + Vite live view (the UI you use)
```

