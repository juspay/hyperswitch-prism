# 10XGRACE

An automated pipeline that turns a connector-integration task description into a working PR against **hyperswitch-prism**. It walks the task through a configurable list of checkpoints — techspec, codegen, cargo build, grpc test, PR review — and retries from a safe point if anything fails.

Three subsystems live inside this workspace:

| Subsystem | What it does | Where it lives |
|---|---|---|
| **Pipeline** | Drives a single connector-integration task through L1→L4 codegen + cargo + grpc + PR. | `packages/core/` (engine, checkpoints) + `packages/dashboard/` (UI) |
| **Parity Autopilot** | Heartbeat-driven prism↔hyperswitch parity loop: sweeps a GitHub root issue, picks the oldest leaf, runs understand/plan/execute, opens/labels PRs. | `packages/parity-core/` |
| **PR Resolver** | Polls a repo for review-thread comments tagged with a trigger, groups by connector, and drives Claude through cargo build/clippy fix loops. | `packages/core/src/pr-resolver/` |

You drive everything from the **dashboard UI** at <http://localhost:3141>. The terminal is only for starting things up.

---

## Setup

One command, fresh clone to running dashboard. Run from the repo root:

```bash
make grace-workspace
```

That target runs `scripts/setup.sh` inside `grace/grace-workspace/`. It probes for prereqs (Node 20+, git, claude, gh, opencode, cargo; sqlite3 CLI is optional — grace's storage uses better-sqlite3's bundled binding), **auto-installs pnpm via corepack if you don't have it yet** (corepack ships with Node ≥ 16.10, so any supported Node already has it), runs `pnpm install` + `pnpm build`, asks which runner you want (claude-code or opencode), detects your claude auth mode from shell env, optionally configures a supplementary LLM HTTP gateway, scaffolds `.env` from `.env.example`, runs a one-shot engine smoke check (also migrates `~/.10xgrace → ~/.tenxgrace` if you're a returning user), and offers to `pnpm dev` you straight into the dashboard at <http://localhost:3141>.

After bootstrap, `cd grace/grace-workspace` and use pnpm for everything else:

- `pnpm dev` — start the supervisor + dashboard + watch-mode rebuilds
- `pnpm check` — read-only re-probe of prereqs + `.env` + runner + auth-mode + gateway consistency. No install, no build, no changes. Run this any time something's off.
- `pnpm test` / `pnpm typecheck` / `pnpm build` — standard workflows

> **Why `make` and not `pnpm` as the entry?** Because `pnpm bootstrap` requires pnpm to invoke. `make` is preinstalled on macOS / Linux / WSL, so a fresh clone needs no JS tooling on PATH before the first command. Once `make grace-workspace` finishes, pnpm is on PATH and is the canonical task runner for everything else.

### Docker (alternative entry)

If you don't want **anything** installed on your host (not even Node), there's a containerized path. From the repo root:

```bash
export TENXGRACE_PROJECT_ROOT=/abs/path/to/target/repo
make grace-workspace-docker
```

That builds a `grace-workspace:latest` image (Node 20, gh, pnpm baked in; claude and opencode are downloaded into a host-side cache at `~/.grace-docker-cli/` on first run and reused thereafter — see the callout below) and starts two compose services: `grace` (supervisor + dashboard at <http://localhost:3141>) and `opencode-serve` (sidecar; only relevant if you're on the opencode runner). The image is built once and re-used across runs.

**Auth flow inside the container** — the same three claude modes work, just delivered via the docker-compose env passthrough + volume mounts:

| Scenario | What you set up on the host | What docker-compose forwards |
|---|---|---|
| A (OAuth)   | `claude /login` once → populates `~/.claude/.credentials.json` | Bind-mount `~/.claude/` read-write so the container's claude reads the same credentials *and* can persist OAuth-token refreshes back to the host |
| B (API key) | `export ANTHROPIC_API_KEY=…` in your shell rc | Env passthrough |
| C (LiteLLM) | `export ANTHROPIC_BASE_URL/_AUTH_TOKEN/_MODEL` in your shell rc | Env passthrough |
| D/E (opencode) | `opencode providers login` once → populates `~/.local/share/opencode/auth.json` | Sidecar bind-mounts `~/.local/share/opencode/:ro`; grace reaches the sidecar at `http://opencode-serve:4096` (env `TENXGRACE_OPENCODE_ATTACH`) |

**When to choose Docker vs `make grace-workspace`**:

- **`make grace-workspace`**: faster iteration (hot reload via `pnpm dev`), smaller surface area, but needs Node 20+, claude, gh, opencode on your host.
- **`make grace-workspace-docker`**: zero host install beyond Docker itself, but requires `docker compose build` after every source edit. Good for trying grace without committing to a native toolchain.

**Limitations** of the Docker path (v1):

- **No hot reload.** Editing source means `docker compose down && make grace-workspace-docker` again. Use native if you're actively developing grace itself.
- **First `docker compose up` adds ~60–90 s** for the one-time linux-arm64 claude + opencode download into `~/.grace-docker-cli/`. Cached for subsequent ups.
- **`TENXGRACE_PROJECT_ROOT` must be exported** before `docker compose up` — it's bind-mounted at the same absolute path inside the container so session worktree paths stored in SQLite resolve identically on both sides.
- **macOS Docker Desktop**: standard bind-mount performance applies; first run is slow.

> **Why path-mirroring (e.g. `${HOME}/.tenxgrace:${HOME}/.tenxgrace`)?** Grace stores absolute paths in SQLite (`/Users/x/.tenxgrace/sessions/abc/proj`). If host and container see different paths for the same dir, sessions silently break — the path stored by one side doesn't resolve on the other. Mounting at the same absolute path on both sides sidesteps the issue without an engine refactor.

> **Host-cached CLIs (`~/.grace-docker-cli/`).** The `claude` and `opencode` binaries are **not** baked into the image — they're downloaded into `~/.grace-docker-cli/` on the host on first `docker compose up` (~60–90 s, only once) and reused thereafter. The cache holds *linux* builds (the container is linux, your host claude/opencode are darwin so they can't be mounted directly); versions are pinned to claude `@2` and opencode `1.3.10` inside `scripts/grace-docker-entrypoint.sh`. To force a re-fetch (e.g. after bumping the pin): `rm -rf ~/.grace-docker-cli && docker compose up`.

### Which runner + auth combo is right for you?

There are two layers to grace's LLM access: the **runner** (claude-code or opencode — drives tool-using agents) and the **supplementary LLM gateway** (optional `llm.*` block for direct HTTP calls from non-tool checkpoints). The five canonical combos:

| # | Scenario | Best when… | Setup commands |
|---|---|---|---|
| A | **Claude Code + Anthropic OAuth** (open-source default) | You have a Claude.ai login. Simplest path. | `claude /login` once, then `make grace-workspace` (accept all defaults — runner stays claude-code, no gateway needed) |
| B | **Claude Code + bare `ANTHROPIC_API_KEY`** | You have a raw Anthropic API key, no OAuth. | `export ANTHROPIC_API_KEY=…` in your shell rc, then `make grace-workspace` (it detects the key and confirms) |
| C | **Claude Code + LiteLLM gateway** (Juspay internal) | Your org runs a LiteLLM proxy (Juspay's `grid.ai.juspay.net`, OpenRouter, etc.) and you want claude CLI routed through it. | Add to shell rc: `export ANTHROPIC_BASE_URL=https://grid.ai.juspay.net/ ANTHROPIC_AUTH_TOKEN=<key> ANTHROPIC_MODEL=<litellm-model-id> CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS=1`. Then `make grace-workspace` — it auto-detects "claude auth mode: LiteLLM gateway" and confirms. |
| D | **Opencode + opencode auth** | You use opencode's bundled auth (OpenAI / OpenRouter / local). | `opencode auth` once, then `make grace-workspace` (pick opencode at the runner prompt), then `opencode serve` in a sidecar |
| E | **Opencode + corporate HTTP gateway** | Mixed corporate setup with opencode for tool-use and a separate gateway for direct LLM calls. | `opencode auth` + `make grace-workspace` (pick opencode + answer "y" to the gateway question + paste baseUrl/model/key) + `opencode serve` sidecar |

> **`claude-code` requires `claude /login` or env-redirect — env-only `ANTHROPIC_API_KEY` works too, but a raw HTTP gateway key alone won't drive it.** If you want a key-only auth flow, use `opencode` instead.

> **How auth flows through grace** — grace spawns the `claude` CLI as a subprocess with `env: process.env`. So whatever you `export` before `pnpm dev` (claude's OAuth keychain state, `ANTHROPIC_API_KEY`, or `ANTHROPIC_BASE_URL`+`ANTHROPIC_AUTH_TOKEN`+`ANTHROPIC_MODEL`) is what the claude CLI sees inside grace. **If you change your shell env, restart `pnpm dev`** — child processes inherit at spawn time.

### What `make grace-workspace` won't do for you

It detects but **does not auto-install** anything except pnpm itself. For everything else (Node.js, claude CLI, gh, opencode, Rust) it prints an OS-specific install hint and exits — auto-installing third-party tooling crosses into "doing things to your machine you didn't ask for" territory. Node in particular is a hard prereq because cross-platform node install (nvm / fnm / asdf / brew / apt) is too varied to script reliably.

It also won't run authentication flows. After install, you'll separately need:
- `claude /login` if you're using Anthropic OAuth (Scenario A)
- `opencode auth` if you're on opencode (Scenarios D/E)
- `gh auth login` if you'll use Parity Autopilot or the PR Resolver
- Shell-rc exports for `ANTHROPIC_BASE_URL` etc. for Scenarios B/C

### When you're using opencode

Open a sidecar terminal and run `opencode serve` — it listens on `http://127.0.0.1:4096`, matching `opencode.attachUrl` in `config.yml`. Keep it open for the whole session. Skip if you're on Claude Code.

### After bootstrap, daily flow

```bash
pnpm dev           # supervisor + dashboard + watch-mode rebuilds
```

Open <http://localhost:3141>, click **+ New Session** to create one, then **Integrate a new connector** or **Standard task** to submit a job. Approve / edit / regenerate at each review gate using the dashboard buttons. The pipeline picks up automatically.

If something looks wrong: `pnpm check` first, then check `~/.tenxgrace/pipeline.sqlite` with `sqlite3` for state. `pnpm clear` is the nuclear option (wipes pipeline DB + sessions).

---

## How to update a step

**Edit a spec at a review gate** (the L2 / L3 / L4 reviews pause and ask you what to do):

- **Approve** — proceed to the next checkpoint.
- **Edit** — open the spec in `$EDITOR` (falls back to nano, then vi). The spec is re-validated when you save. L3 also checks ids, `dependsOn`, and cycles. L4 will ask whether to regenerate code for edited tasks.
- **Regenerate** — type free-text guidance, and the generator runs again with your notes.

**Re-run from an earlier checkpoint**: in the dashboard, click the checkpoint you want to restart from on the run timeline. The engine rewinds and replays from there.

---

## How to remove the database and start fresh

Pipeline state lives in `~/.10xgrace/`; PR-resolver state lives in `~/.tenxgrace/`. To wipe both:

```bash
pnpm clear
```

This removes the pipeline DB (sqlite + WAL/shm), the resume pointer, all per-session worktrees, the PR-resolver worktree, and the PR-resolver state file. Refresh the dashboard — the **Past runs** list will be empty.

Manual fallback:

```bash
rm -rf ~/.10xgrace ~/.tenxgrace
```

---

## When things fail

Every checkpoint has a rollback target. After 3 failed retries (default) the run stops. Tune retry count via `maxRetries` in `config.yml`.

| If this fails | Pipeline rolls back to |
|---|---|
| task / product_alignment | task |
| design_gate | design_gate |
| l2_* / l3_* / l4_* | the matching `_gen` step |
| implementation / compiler | implementation (with code-repair) |
| design_match | implementation |
| cypress / playwright / pr_review | compiler (with code-repair) |
| regression | cypress |

---

## How to add or skip a step

The pipeline runs a fixed list of checkpoints in order. You can **skip** some via config; **adding** a new one is a code change.

### Skip a step (config only)

Only the **regression** checkpoint is fully skippable from `config.yml`:

```yaml
checkpoints:
  regression:
    enabled: false   # checkpoint runs but returns immediately
```

`design_match` is skipped automatically when the **design_gate** stage decides the task doesn't need a visual design — that's runtime, not config.

### Add a new step (code change)

To insert a new checkpoint, edit three files in `packages/core/src/`:

1. **`types.ts`** — add the new id to the `CheckpointId` union.
2. **`checkpoints/<your-step>.ts`** — export a `Checkpoint` object with `id`, `name`, `description`, `retryFrom`, and an async `run(ctx)` function. Use any existing checkpoint as a template — `regression.ts` is the simplest.
3. **`checkpoints/index.ts`** — import your new checkpoint and add it to the `ALL_CHECKPOINTS` array at the position you want it to run.

Then `pnpm build` and your new step is wired in. If your new step belongs in the middle of the pipeline, also update the `retryFrom` of any later step whose rollback should now point at yours.

---

## Project layout

```
hyperswitch-prism/                          # repo root (yarn-configured, NOT pnpm)
└── grace/
    ├── grace-workspace/                    # ← you are here; pnpm workspace root
    │   ├── config.yml                      # all your settings
    │   ├── package.json                    # workspace root: pnpm dev / clear / build
    │   ├── pnpm-workspace.yaml             # packages: packages/*
    │   ├── docs/                           # planning docs (parity-autopilot, adoption, integration)
    │   └── packages/
    │       ├── core/                       # pipeline engine, checkpoints, pr-resolver TS
    │       │   └── src/
    │       │       ├── types.ts            # CheckpointId union — start here to add a step
    │       │       ├── checkpoints/        # one file per checkpoint + index.ts (ALL_CHECKPOINTS)
    │       │       ├── pr-resolver/        # PR Resolver service
    │       │       ├── new-connector/      # flow-detector + flow-runner
    │       │       ├── generators/         # L2/L3/L4 prompt generators
    │       │       ├── tools/              # claude-code / opencode / spawn-agent runners
    │       │       ├── engine.ts           # runs checkpoints, handles retries
    │       │       └── llm.ts              # LLM client
    │       ├── parity-core/                # Parity Autopilot: heartbeat-driven parity loop
    │       │   └── src/{orchestrator,phases,github,verify,dashboard}.ts
    │       ├── cli/                        # the `10xgrace` command (used by `pnpm dev`)
    │       └── dashboard/                  # React + Vite live view
    └── pr-resolver/
        └── prompts/                        # *.md prompts loaded at runtime by core/pr-resolver
```

---

## PR Resolver (optional tab)

The **PR Resolver** tab is an opt-in feature that polls a GitHub repo for review-thread comments tagged with a trigger (default `@HS-prism-bot`), groups them by connector, and drives Claude through a cargo build/clippy fix loop to resolve each one. Output lands as a per-thread reply on GitHub plus a Kanban view in the dashboard.

### Enabling

1. Make sure the `gh` CLI is logged in (`gh auth login`). The resolver shells out for all GitHub access — no token in code.
2. In `.env` (or your shell):
   ```bash
   TENXGRACE_PR_RESOLVER_ENABLED=true
   TENXGRACE_PR_RESOLVER_GITHUB_REPO=juspay/hyperswitch-prism   # or your fork
   TENXGRACE_PR_RESOLVER_TRIGGER=@HS-prism-bot                  # optional
   ```
3. Restart `pnpm dev`. The supervisor boots the resolver in-process, the dashboard sidebar gains a **PR Resolver** entry, and the page connects to live state.

### What lives where

- **`grace/pr-resolver/prompts/*.md`** — resolution and fix-loop prompts. Edit without rebuilding; the loader reads from disk per call. Frontmatter declares the variables.
- **`packages/core/src/pr-resolver/`** — TS port of the service. `service.ts` is the orchestrator with the six freshness gates; `resolver.ts` drives one Claude session per connector sub-task; `cargo-loop.ts` runs `cargo build`/`clippy` and resumes the same Claude session with errors on failure; `worktree.ts` wraps the working clone at `~/.tenxgrace/pr-resolver/worktree/`.
- **`~/.tenxgrace/pr-resolver-state.json`** — persisted thread state (fixed / failed / build_blocked) so PRs don't get retried after they're already resolved.

### CLI

For ad-hoc runs without the supervisor:

```bash
pnpm build
node packages/cli/dist/index.js pr-resolver --once    # one cycle, exit
node packages/cli/dist/index.js pr-resolver           # watch loop
```

### Tweaking the prompts

The prompts are pure Markdown with YAML frontmatter and Mustache placeholders. The loader's contract is documented in `grace/pr-resolver/prompts/README.md`. Common edits:

- Tighten the per-comment scope rule (e.g. "only this single file, not the whole connector").
- Add a `## Examples` section with known-good rewrites for your codebase.
- Swap the answer-instruction tone at the end.

No restart needed — the next cycle re-reads the file.
