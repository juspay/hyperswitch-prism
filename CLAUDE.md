# CLAUDE.md — hyperswitch-prism (UCS)

Rust connector-service monorepo. This file is a **router**: it points at the one canonical
entry point per job and records the repo facts agents get wrong. It does not restate procedures.

## Routing table — one entry point per job

| Job | Canonical entry point |
|---|---|
| Implement a new connector from scratch | skill `new-connector` (`.skills/new-connector/`) — needs a spec first |
| Generate a technical specification | skill `generate-tech-spec` (`.skills/generate-tech-spec/`) |
| Add a flow to an existing connector | skill `add-connector-flow` (`.skills/add-connector-flow/`) |
| Add a payment method to an existing connector | skill `add-payment-method` (`.skills/add-payment-method/`) |
| Review a PR | skill `pr-reviewer` (`.skills/pr-reviewer/`) |
| Connector coverage / capability metrics | skill `coverage-report` (`.skills/coverage-report/`) |
| Embed prism into an app | skill `demo-integration` (`.skills/demo-integration/`) |
| Use the Prism SDKs (Python/Node/Java/Rust) | skill `sdk-integration` (`.skills/sdk-integration/`) |
| Batch / multi-connector run (one flow across many connectors) | `grace/workflow/1_orchestrator.md` (legacy; see note below) |

Each skill also has a `make` launcher: `make new-connector`, `make gen-tech-spec`,
`make add-flow`, `make add-payment-method`, `make review-pr` — they just open an AI editor
on that skill, so invoking the skill directly is equivalent.

## GRACE is mid-migration — pick the right fork

Three overlapping copies of the same procedures exist. In order of precedence:

1. **`.skills/*/SKILL.md` — current.** Use these. Shared references live in `.skills/_shared/references/`.
   **Fallback rule:** `.skills/` is the narrow fork. When it has no pattern for what you need,
   `grace/rulesbook/codegen/guides/patterns/` is the wider **authoritative** set — flows at
   `pattern_<flow>.md`, per-payment-method at `authorize/<category>/pattern_authorize_<category>.md`.
   Read it rather than inventing a pattern. Several `.skills` reference files are already
   symlinks into it; edit the rulesbook target, never the symlink.
2. **`grace/rulesbook/codegen/.gracerules`, `.gracerules_add_flow`, `.gracerules_add_payment_method`** —
   kept for non-Claude agents (Cursor, Windsurf, opencode, codex). Do not follow them when a skill covers the job.
3. **`grace/workflow/*.md` — legacy prompt-driven path.** Stale relative to the codebase. Only
   `1_orchestrator.md` still has a unique job (batch/multi-connector runs); treat the rest as historical.

## Repo facts

- **There is no `crates/connector-integration/`.** The real path is
  `crates/integrations/connector-integration/`.
- **Connectors** live in `crates/integrations/connector-integration/src/connectors/` — one
  `<name>.rs` module per connector (111 today; `macros.rs` in that dir is shared macros, not a
  connector), most with a `<name>/transformers.rs` alongside. Registration:
  `src/connectors.rs`; non-card variants in `payout_connectors/`, `authenticator_connectors/`,
  `surcharge_connectors/`.
- **Base URLs** per environment: `config/development.toml`, `config/sandbox.toml`, `config/production.toml`.
- **Flow patterns**: `.skills/_shared/references/flow-patterns/` covers the 6 core flows plus
  `webhook.md`. Everything else (disputes, mandates, payouts, 3DS, order-create, tokenization,
  incremental auth) is only in `grace/rulesbook/codegen/guides/patterns/` — see the fallback rule above.
- **Payment-method patterns**: `.skills/add-payment-method/references/payment-method-patterns/`
  has a file for all 21 `PaymentMethodData` variants; the ones with no condensed skill-local
  version are symlinks into `grace/rulesbook/codegen/guides/patterns/authorize/`.
- **Webhooks are not a `ConnectorIntegrationV2` flow.** No `connector_flow` marker, no macro —
  a plain `impl IncomingWebhook`, required by `ConnectorServiceTrait`. Two RPCs, two phases:
  `EventService.ParseEvent` (stateless: `get_event_type` + `get_webhook_event_reference`) then
  `EventService.HandleEvent` (secrets: `verify_webhook_source` + `process_*_webhook`).
- **Tech specs** land at `grace/rulesbook/codegen/references/<connector>/technical_specification.md`
  (the `new-connector` skill reads this path). The grace CLI's own output goes to
  `grace/rulesbook/codegen/references/specs/<Connector>.md`.
- **Test specs / scenarios**: `crates/internal/integration-tests/src/connector_specs/<connector>/specs.json`.
- **Credentials** resolve in this order: `CONNECTOR_AUTH_FILE_PATH` → `UCS_CREDS_PATH` →
  `creds.json` at repo root. `creds_dummy.json` is the placeholder template; never commit real creds.
- **Other crates**: `crates/grpc-server/grpc-server` (server), `crates/types-traits/grpc-api-types`
  (protos under `proto/`), `crates/types-traits/domain_types`, `crates/common/`, `crates/internal/`.
- Generated coverage matrix: `docs-generated/all_connector.md` (via `make docs`).

## Build / test / validate

```
make fmt            # cargo +nightly fmt --all
make check          # cargo hack check --each-feature --no-dev-deps
make clippy         # cargo hack clippy --each-feature --no-dev-deps
make test           # cargo hack test --each-feature
make nextest        # cargo nextest run --config-file .nextest.toml
make check-specs    # cargo run --bin check_connector_specs (flow→suite parity)
make all            # fmt → check → clippy → test
make ci             # same, warnings = errors
```

Connector integration tests (run `make setup-connector-tests` once first):

```
make test-connector connector=stripe [interface=sdk|grpc]
make test-scenario connector=stripe suite=authorize scenario=no3ds_auto_capture_credit_card
```

Before pushing: `make validate-pre-push` (`scripts/validation/pre-push.sh`; `-full` adds tests,
`-fix` auto-fixes). Spell check is CI job `typos` (crate-ci/typos, config `./.typos.toml` — note
the leading dot; the root `typos.toml` is not the one CI uses). CI gate jobs in
`.github/workflows/ci.yml`: `typos`, `clippy`, `formatting`, `auto-fix`, `check`, `test`, `sdk-test`.

## Commit & PR conventions

- Conventional commits, enforced on PR titles by `cog verify`
  (`.github/workflows/pr-convention-checks.yml`). Allowed types from `cog.toml`:
  `feat`, `fix`, `perf`, `refactor`, `test`, `docs`, `proto`, `chore`, `build`, `revert`, `ci`.
- Connector work uses scope `connector` with a bracketed connector name:
  `fix(connector): [PayNearMe] RSync must not terminally fail a refund`.
  Other scopes in use: `framework`, `core`, `proto`, `payouts`, or a bare connector name.

Hard rules (all have been violated in practice):

- **Never `git add -A` / `git add .`** — stage from an explicit file manifest you wrote down.
  Untracked scratch files (`task*.json`, `grace/*_report.md`, PDFs) live in this tree.
- **Never force-push.**
- Never commit `creds.json` or anything under `target/`, `.claude/worktrees/`, `.ct-worktrees/`.
