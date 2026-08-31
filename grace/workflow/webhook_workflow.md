# IncomingWebhook Implementation Workflow (hyperswitch → hyperswitch-prism / UCS)

A Grace-style, fully-autonomous workflow for **implementing incoming-webhook support for a payment
connector in hyperswitch-prism (UCS)**, porting behaviour 1:1 from **hyperswitch (the Direct
gateway)** as the **single source of truth** (use webfetch if needed), then validating it **end-to-end in BOTH shadow mode and
primary mode** before opening a PR on `juspay/hyperswitch-prism`.

This file is the orchestrator. It follows the same architecture as `grace/workflow/1_orchestrator.md`
(orchestrator → connector agent → phase subagents), the same hard guardrails (strictly sequential,
autonomous, scoped git, no creds), and reuses the shadow stack documented in
`~/hyperswitch-prism/hyperswitch/shadow.md` (read it before running — §1 topology, §2 setup, §4 gotchas).

---

## 0. The one rule that governs everything: hyperswitch is the source of truth

For a given connector, **whatever the hyperswitch Direct-gateway `IncomingWebhook` implementation does
is correct by definition.** Prism's job is to reproduce it — same event-type mapping, same object
reference, same signature scheme, same status/content. Every comparison in this workflow is
`prism (UCS) vs hyperswitch (Direct)`; prism is never the reference.

- **Source of truth (hyperswitch):**
  - Trait: `~/hyperswitch-prism/hyperswitch/crates/hyperswitch_interfaces/src/webhooks.rs` (`IncomingWebhook`).
  - Per-connector impl: `~/hyperswitch-prism/hyperswitch/crates/hyperswitch_connectors/src/connectors/{connector}.rs` (+ `{connector}/transformers.rs`).
  - Event enum: `api_models::webhooks::IncomingWebhookEvent` (`crates/api_models/src/webhooks.rs`).
  - Dispatch order (Direct gateway): `crates/router/src/core/webhooks/gateway.rs` → `decode_webhook_body` → `get_webhook_object_reference_id` → `get_webhook_event_type` → `verify_webhook_source` → `get_webhook_resource_object`.
- **Target (prism / UCS):**
  - Trait: `~/new-grpc/hyperswitch-prism/crates/types-traits/interfaces/src/connector_types.rs` (`IncomingWebhook`).
  - Per-connector impl: `~/new-grpc/hyperswitch-prism/crates/integrations/connector-integration/src/connectors/{connector}.rs` (+ `{connector}/transformers.rs`).
  - Two-phase gRPC surface: `EventService/ParseEvent` (pre-credential) then `EventService/HandleEvent` (post-credential) — proto in `crates/types-traits/grpc-api-types/proto/{payment,services}.proto`.
  - Registry: `crates/integrations/connector-integration/src/types.rs` (`convert_connector`); default verify impl macro in `default_implementations.rs`.

> ⚠️ The exact prism trait method names/signatures have drifted historically (e.g. `RequestDetails`
> vs `IncomingWebhookRequestDetails`, `get_event_type` vs `get_webhook_event_type`). **Never hardcode
> signatures from this doc or the pattern files.** The implementation subagent MUST open the live trait
> definition and an existing reference connector in prism and match them exactly.

**Reference connectors that already implement webhooks in prism** (use the closest signature-scheme
match as the template): `adyen`, `ppro`, `bluesnap`, `noon`, `novalnet`, `cryptopay`, `revolut`,
`trustpay`, `fiuu`, `payload`, `authorizedotnet`, `paypal` (out-of-band), `truelayer` (out-of-band).

**Grace pattern guides (read before implementing):**
- `grace/rulesbook/codegen/guides/patterns/pattern_IncomingWebhook_flow.md` — full webhook pipeline (event mapping, content processing).
- `grace/rulesbook/codegen/guides/patterns/pattern_verify_webhook_source.md` — signature verification (in-band vs out-of-band families, scheme table per connector).

---

## 1. Inputs

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{CONNECTOR}` / `{CONNECTORS_FILE}` | A single connector (exact casing) OR a JSON array of names | `Bluesnap` / `["Bluesnap","Cryptopay"]` |
| `{BRANCH}` | Single branch all work is committed on | `feat/ucs-webhooks` |
| `{EVENTS}` | (Optional) which webhook families to cover; default = whatever hyperswitch implements | `payment,refund,dispute` |
| `{PRISM_ROOT}` | Prism repo (build/run/edit target) | `~/new-grpc/hyperswitch-prism` |
| `{HS_ROOT}` | Hyperswitch repo (source of truth + shadow router) | `~/hyperswitch-prism/hyperswitch` |
| `{VALIDATION_ROOT}` | Shadow validation/mitm stack | `~/ucs-shadow-validation-service` |

Connector names: exact casing for display / `ConnectorEnum` / `x-connector` header; lowercase for files,
branches, paths. Credentials + webhook secrets live in `creds.json` at the prism repo root (see shadow.md §2.5).

---

## 2. HARD GUARDRAILS (read once, apply everywhere)

1. **STRICTLY SEQUENTIAL, NEVER PARALLEL.** Process ONE connector at a time. One Task call per message,
   wait for the result, then the next in a NEW message. The grpc server (`:8000`) and the shadow stack
   are shared singletons — concurrent connectors corrupt the branch and the diff store. (shadow.md §3, §4.1)
2. **FULLY AUTONOMOUS — NEVER STOP OR ASK.** No "Option A / Option B", no "should I continue?". Missing
   creds → skip. Ambiguity → best judgement + proceed. Partial failure → record it and move on.
3. **hyperswitch is the source of truth** (§0). Do NOT invent webhook behaviour. If prism would diverge
   from hyperswitch, prism is wrong — fix prism.
4. **STRICT, END-TO-END GATES.** A connector is SUCCESS only if **build passes AND primary-mode grpcurl
   passes AND shadow-mode diff is empty**. All three. No exceptions. A green build proves syntax only.
5. **NO LOOPING WITHOUT A FIX.** Never re-run a build / grpcurl / webhook fire without changing code
   first. Read server + router logs before every fix. 3-strike rule on identical errors. Max 7
   build+test iterations per connector. Maintain a fix log (error → file → change → why). (mirrors `2.3_codegen.md`)
6. **SCOPED GIT.** Only stage `crates/integrations/connector-integration/src/connectors/{connector}*`
   in prism. Never `git add -A`. Never commit `config/development.toml`, `creds.json`, `.env`, or the
   mitm CA cert. Never force-push `main`.
7. **NO CREDS / SECRETS IN CODE OR PR.** Connectors read secrets at runtime; never hardcode webhook
   secrets, API keys, or sample signatures. Scrub the diff before pushing.
8. **PRESERVE LOCAL CONFIG.** `config/development.toml` (both repos) carries the proxy + comparison +
   mitm-CA config the shadow stack needs and shows as `M` in git status — keep it, never reset/commit it
   (shadow.md §3.A).
9. **NO `cargo test` for connector validation.** Validation is grpcurl (primary) + shadow diff. Unit
   tests in the connector file are allowed if the reference connector has them, but they are not the gate.

---

## 3. Architecture

```
webhook_workflow.md (orchestrator — this file)
  └── for each connector, SEQUENTIAL:
        Connector Agent (spawned via Task, subagent_type="general-purpose")
          ├── Phase 0  Preflight & stack health        (self)
          ├── Phase 1  Source-of-truth extraction       (subagent → reads hyperswitch)
          ├── Phase 2  Prism gap analysis               (self)
          ├── Phase 3  Implement in prism               (subagent → writes prism)
          ├── Phase 4  Build prism                       (self / subagent)
          ├── Phase 5  PRIMARY-mode E2E test (grpcurl)   (subagent, strict)
          ├── Phase 6  SHADOW-mode E2E test (full stack) (subagent, strict)
          └── Phase 7  Commit & PR                       (subagent)
```

The orchestrator does pre-flight, credential checks, and coordination only. It spawns ONE Connector
Agent per connector and waits. It does NOT read hyperswitch code, edit prism, build, run grpcurl, fire
webhooks, or create PRs itself.

---

## 4. Orchestrator steps

### STEP 0 — Discover connectors
If `{CONNECTORS_FILE}` is a JSON array: `cat {CONNECTORS_FILE} | jq -r '.[]'`. If a single `{CONNECTOR}`
was passed, the list has one entry. Store as `CONNECTOR_LIST` (authoritative — every entry must be covered).

### STEP 1 — One-time stack pre-flight (per shadow.md §1–§2)
Do NOT start per-connector work until ALL are green:
```bash
# Prism repo to latest main + build
git -C {PRISM_ROOT} checkout main && git -C {PRISM_ROOT} pull --ff-only origin main
git -C {PRISM_ROOT} checkout -b {BRANCH}
cargo build -p grpc-server  --manifest-path {PRISM_ROOT}/Cargo.toml      # (run from {PRISM_ROOT})
# Hyperswitch repo (source of truth + shadow router) on main, router built
git -C {HS_ROOT} checkout main && git -C {HS_ROOT} pull --ff-only origin main   # keep config/development.toml (M)
cargo build -p router --manifest-path {HS_ROOT}/Cargo.toml
# Shadow validation stack (mitm :8081, web :8083, compare :9000) — shadow.md §2.4
cd {VALIDATION_ROOT} && docker compose up -d
# Health: ports 8000(prism gRPC), 8080(router), 8081/8083/9000(validation), 5432(pg), 6380(redis)
lsof -i:8000 -i:8080 -i:8081 -i:9000 ; docker ps
# Pending migrations cause HE_00 500s — check first (shadow.md §4.11)
diesel migration list --database-url postgres://db_user:db_pass@localhost:5432/hyperswitch_db --migration-dir {HS_ROOT}/migrations
```
Confirm `ucs_enabled=true` exists in the DB (`SELECT key FROM configs WHERE key='ucs_enabled';`). If
missing: `POST :8080/configs/ {"key":"ucs_enabled","value":"true"}` (header `x-tenant-id: public`,
`api-key: test_admin`).

For each connector in `CONNECTOR_LIST`: if it has no entry in `creds.json`, mark **SKIPPED (no
credentials)** and remove from the list. Proceed silently.

### STEP 2 — For each connector (sequential, ONE Task per message)
```
Task(
  subagent_type="general-purpose",
  description="Implement IncomingWebhook for {CONNECTOR} in prism",
  prompt="Read and follow grace/workflow/webhook_workflow.md, the 'Connector Agent' section.

Variables:
  CONNECTOR: <exact casing>
  BRANCH: {BRANCH}
  EVENTS: <payment,refund,dispute or empty=match hyperswitch>
  PRISM_ROOT: {PRISM_ROOT}
  HS_ROOT: {HS_ROOT}
  VALIDATION_ROOT: {VALIDATION_ROOT}"
)
```
Wait for the result (`SUCCESS | FAILED | SKIPPED` + PR url). Only then spawn the next connector in a new
message. Stay on `{BRANCH}` the whole time.

### AFTER ALL CONNECTORS — Report
```
=== WEBHOOK IMPLEMENTATION SUMMARY ===
Branch: {BRANCH} | Total: <n> | Success: M | Failed: K | Skipped: S
Per connector: {connector}: STATUS | primary: PASS/FAIL | shadow: keyDiff {} / <diff> | PR: <url> | reason
```

---

## 5. Connector Agent

You own ONE connector end-to-end. Read this whole section first. Spawn subagents (Task tool) for the
heavy phases (1, 3, 5, 6, 7); do the light phases (0, 2, 4) yourself. Build → primary test → shadow test
→ commit is a hard gate chain — never skip ahead.

### Phase 0 — Preflight & branch
```bash
cd {PRISM_ROOT} && pwd && ls Cargo.toml crates/ Makefile
git -C {PRISM_ROOT} branch --show-current        # must be {BRANCH}; if not, checkout (do NOT create new)
cat creds.json | jq '.["{connector}"]'           # must exist (orchestrator already filtered)
```
If the connector already implements `IncomingWebhook` in prism AND has webhook secrets wired, this may
be a **gap-fill** (missing event family / wrong mapping) rather than a fresh implementation — Phase 2
decides. If files are missing entirely → SKIPPED (connector not integrated yet; webhooks need an
existing connector).

### Phase 1 — Source-of-truth extraction (SPAWN SUBAGENT — reads hyperswitch ONLY)
Goal: produce a precise **Webhook Behaviour Spec** from the hyperswitch Direct implementation. This spec
is the contract prism must satisfy.

Spawn a subagent to read `{HS_ROOT}/crates/hyperswitch_connectors/src/connectors/{connector}.rs` +
`{connector}/transformers.rs` + the `IncomingWebhook` trait, and return:
1. **Signature scheme**: algorithm (HMAC-SHA256/512, SHA256, MD5, JWS, or out-of-band API like PayPal),
   where the signature lives (which header / body field), the exact **message construction** (byte-for-byte:
   delimiters, timestamp prefix, field order), and encoding (hex/base64). Quote `get_webhook_source_verification_{algorithm,signature,message}` and any `verify_webhook_source` override.
2. **Body type(s)**: the struct(s) the webhook deserializes into; JSON vs form-urlencoded vs XML.
3. **Event-type mapping**: the connector's native event enum → `IncomingWebhookEvent`, EVERY arm. Note
   conditional logic (e.g. status field preferred over event code; chargeback vs payment branching).
4. **Object reference**: how `get_webhook_object_reference_id` decides PaymentId vs RefundId vs Dispute
   vs Mandate vs Payout, and which field feeds the id (merchant ref vs connector/psp ref).
5. **Per-family content**: for payment/refund/dispute, the status mapping
   (→ AttemptStatus / RefundStatus / DisputeStatus), amount/currency extraction, error code/message,
   mandate ref, network txn id.
6. **Out-of-band?** If the connector verifies via its own API (PayPal `verify-webhook-signature`,
   TrueLayer JWKS), flag it — prism uses the Family-2 path (see pattern_verify_webhook_source.md §Family 2).
7. **A real sample webhook payload** for each event family (from connector docs / the connector's
   test fixtures in hyperswitch) — needed to build the test vectors in Phases 5–6.

Output: a `Webhook Behaviour Spec` markdown block (keep it in the agent's working notes; do not commit it).

### Phase 2 — Prism gap analysis (self)
Read the live prism trait (`crates/types-traits/interfaces/src/connector_types.rs` → `IncomingWebhook`)
and the **closest reference connector** by scheme (§0 list). Confirm:
- Which trait methods are required vs defaulted in the CURRENT prism trait (copy signatures from source).
- Whether the proto `WebhookEventType` enum already has every event the spec needs
  (`crates/types-traits/grpc-api-types/proto/payment.proto`). If an event type is missing from the proto,
  that is a **proto change** (two-repo, follows shadow.md §3.D "Proto change" rev-pin process) — flag it
  and prefer mapping to the nearest existing event if semantically safe; otherwise escalate in the report.
- How webhook secrets reach the connector (proto `WebhookSecrets` → `ConnectorWebhookSecrets` via
  `foreign_try_from`) and whether the connector is in/out of the `default_impl_verify_webhook_source_v2!`
  macro list in `default_implementations.rs`.
- Whether the connector is registered in `types.rs::convert_connector` (it must be, since the connector
  already exists).

Classify: **(a) fresh webhook impl**, **(b) gap-fill** (add missing family / fix mapping), **(c) proto
gap** (needs a new event type → two-repo, heavier). Record the classification.

### Phase 3 — Implement in prism (SPAWN SUBAGENT — writes prism ONLY)
Hand the subagent: the Webhook Behaviour Spec (Phase 1), the gap classification (Phase 2), the reference
connector path, and both pattern guides. Rules for the subagent:
- **Match the live prism trait + reference connector exactly** — read them, copy the method signatures,
  do not trust signatures from any doc.
- Port the hyperswitch behaviour faithfully: same scheme, same byte-exact message, same event map, same
  object reference, same status mapping. Where prism's domain enums differ from hyperswitch's, map to the
  semantic equivalent.
- Define webhook body structs + event enum in `{connector}/transformers.rs`; implement the trait in
  `{connector}.rs`. Family-1 (in-band crypto) for 95% of connectors; Family-2 (out-of-band) only if the
  hyperswitch connector is out-of-band (then also register in the external-verification config and remove
  from the default-impl macro — see pattern_verify_webhook_source.md §Family 2 steps 9–10).
- Use `common_utils::crypto::{HmacSha256, HmacSha512, Sha256, Md5}`. **Never modify the raw body bytes
  before HMAC** (parse/re-serialize breaks digests). Decode the signature (hex/base64) before comparing.
- ONLY edit files under `crates/integrations/connector-integration/src/connectors/{connector}*`. Relative
  paths. Do NOT run cargo build (Phase 4 does). Do NOT commit.

### Phase 4 — Build prism (self)
```bash
cd {PRISM_ROOT} && cargo build -p grpc-server 2>&1
```
On failure: read the error, fix the specific file, rebuild. Anti-loop rules apply (Guardrail 5). If the
error is outside the connector's files and unfixable after reading logs → FAILED.

### Phase 5 — PRIMARY-mode end-to-end test (SPAWN SUBAGENT — strict)
Test prism's webhook directly over gRPC and assert it reproduces the hyperswitch source-of-truth outputs.

Start a clean server (kill 8000/8080 first — shadow.md, `2.3_codegen.md` step 2):
```bash
lsof -ti:8000 | xargs kill -9 2>/dev/null || true ; lsof -ti:8080 | xargs kill -9 2>/dev/null || true
cd {PRISM_ROOT} && (make stop-grpc 2>/dev/null || true) && cargo run --bin grpc-server &
for i in $(seq 1 80); do sleep 2; grpcurl -plaintext localhost:8000 list && break; done
```
For each event family in the spec, run BOTH phases with the **real sample payload** from Phase 1 and the
webhook secret from `creds.json`:

**(a) ParseEvent** (pre-credential — event type + reference):
```bash
grpcurl -plaintext -H 'x-connector: {connector}' -d '{
  "request_details": { "method": 2, "uri": "/webhooks/{connector}", "headers": {<signature+ts headers>},
    "body": "<RAW webhook body, JSON-escaped>" }
}' localhost:8000 types.EventService/ParseEvent
```
**(b) HandleEvent** (post-credential — verification + content):
```bash
grpcurl -plaintext -H 'x-connector: {connector}' -d '{
  "request_details": { ... same as above ... },
  "webhook_secrets": { "secret": "<from creds.json>" },
  "event_context": { "payment": { "capture_method": 2 } }
}' localhost:8000 types.EventService/HandleEvent
```
(Method numbers / field names: confirm against the live proto. The runner may need `x-merchant-id`.)

**STRICT pass criteria (ALL must hold, for every event family):**
- ParseEvent `event_type` == the hyperswitch-mapped event for that payload (from the Phase-1 spec).
- ParseEvent `reference` resolves to the correct id kind + value (payment vs refund vs dispute) matching
  hyperswitch's `get_webhook_object_reference_id`.
- HandleEvent `source_verified: true` with the correct secret.
- HandleEvent `event_content` carries the right `*_response` with the **status matching hyperswitch's
  mapping** (e.g. AUTHORIZED/CHARGED/FAILED), correct amount/currency, and ids.
- No `Error invoking method`, no `UNIMPLEMENTED`/`INTERNAL`, no `source_verified:false` on a valid signature.

**STRICT negative tests (must also pass):**
- Tampered body (flip one byte) OR wrong secret → `source_verified: false` (never `true`, never error-out).
- Missing signature header → graceful error / `false`, not a panic.

On any failure: read the grpc-server logs FIRST (the grpcurl response is too vague), find the root cause
(message construction off by a delimiter, wrong header name, encoding, event-arm gap), fix the connector
code, **go back to Phase 4 (rebuild)**, retest. Anti-loop rules apply. Capture the full grpcurl
command(s) + response(s) (redact secrets) as `PRIMARY_EVIDENCE`.

### Phase 6 — SHADOW-mode end-to-end test (SPAWN SUBAGENT — strict; hyperswitch = ground truth)
This is the definitive check: fire a **real webhook through the hyperswitch router** so the Direct
(hyperswitch) gateway and the shadow UCS (prism) run **on the same request**, and assert they agree.
This uses the shadow webhook path in `{HS_ROOT}/crates/router/src/core/webhooks/gateway.rs`
(`execute_incoming_webhook_gateway` → `ShadowUnifiedConnectorService` arm → `spawn_shadow_ucs_run` →
`report_shadow_diff`). The shadow comparison is on the **parsed outcome** (event_type, source_verified,
content_kind, reference) — `WebhookShadowSnapshot` — not an outbound request_diff (webhooks are inbound).

**Setup (per shadow.md §2–§3):**
1. Ensure the full stack is up (router :8080, prism :8000, mitm :8081, compare :9000) and `config/development.toml`
   in both repos still has `[proxy]` + `[comparison_service]` (enabled, url `:9000/.../router-data`) + mitm CA.
2. Ensure the connector has a merchant + MCA in `hyperswitch_db` **with the webhook secret configured**
   on the MCA. **This is mandatory** — `source_verified` only matches if prism's secret equals what the
   Direct gateway uses (`build_webhook_secrets_from_merchant_connector_account`). A missing/mismatched
   secret shows up as a `source_verified` diff that is a *test-setup* bug, not a code bug.
3. Enable webhook shadow for this connector (flow name is literally `Webhooks`):
   ```
   POST :8080/configs/  {"key":"ucs_enabled","value":"true"}
   POST :8080/configs/  {"key":"ucs_rollout_config_<merchant_id>_<connector_or_mca_id>_Webhooks",
     "value":"{\"rollout_percent\":1.0,\"http_url\":\"http://localhost:8081\",\"https_url\":\"http://localhost:8081\",\"execution_mode\":\"shadow\"}"}
   ```
   (header `x-tenant-id: public`, admin api-key for configs.)
4. Restart the router so it loads the rollout/proxy config; reconnect note: first UCS call after a prism
   restart may `Cancelled` (shadow.md §4.10) — re-fire once.

**Fire the webhook** (raw connector body + the connector's signature/timestamp headers, from Phase 1):
```bash
curl -sS -X POST 'http://localhost:8080/webhooks/<merchant_id>/<connector_id_or_name>' \
  -H 'content-type: application/json' \
  -H '<signature header>: <valid sig over the body>' -H '<timestamp header>: <ts>' \
  --data-binary @sample_webhook_body.json
```
Confirm in the router log: `execution_path=ShadowUnifiedConnectorService` and a `"Webhook shadow diff"`
line.

**STRICT pass criteria:**
- `"Webhook shadow diff"` log shows `event_type_match=true`, `primary_source_verified == shadow_source_verified`
  (both `true` for a valid signature), and the reference + content_kind match.
- If `[comparison_service]` is on, the validation service records no diff:
  ```
  GET :9000/validation-service/api/results            # find the latest key for this x-request-id
  GET :9000/validation-service/api/results/<key>      # require bodyComparison.keyDiff == {} (only ignore-listed headers remain)
  ```
- Run for EVERY event family covered (payment success/failure, refund, dispute as applicable).

**Then prove PRIMARY mode through the router** (prism actually serves the webhook live, not just shadow):
flip the same rollout key to `"execution_mode\":\"primary\"`, restart the router, re-fire the webhook,
and confirm the router returns the correct ack/2xx and the live path went through UCS
(`ExecutionPath::UnifiedConnectorService`) with the same event resolution. This is the "primary mode end
to end" leg.

On any mismatch: the diff IS the bug (or a setup issue per step 2). Read the router log + prism log,
decide whether it's (i) a prism mapping/scheme bug → fix prism, rebuild (Phase 4), rerun Phases 5–6;
(ii) a missing/benign field (idempotency key, timestamp, `x-merchant-id`) → add to the validation
ignore-list (shadow.md §1 layer) and document; (iii) a secret/setup issue → fix the MCA secret. Anti-loop
rules apply. Capture the `"Webhook shadow diff"` log + the `:9000` result (before/after) as `SHADOW_EVIDENCE`.

### Phase 7 — Commit & PR (SPAWN SUBAGENT)
Mirror `grace/workflow/2.4_pr.md` exactly, adapted for webhooks. Target repo is **`juspay/hyperswitch-prism`**
(origin), commit on `{BRANCH}`, no branch creation / cherry-pick.
- Stage ONLY `crates/integrations/connector-integration/src/connectors/{connector}*`
  (plus `default_implementations.rs` / proto files only if a Family-2 or proto-gap change required it — call it out).
- **Credential scrub (mandatory):** scan the diff for hardcoded secrets, signatures, API keys, sample
  body values that match real creds → remove. Verify `config/development.toml`, `creds.json`, `.env`,
  mitm CA are NOT staged.
- Commit: `feat(connector): implement {connector} incoming webhooks in UCS`.
- **Before checking for an existing PR**, follow shadow.md §F step 0 / `1_orchestrator.md` rule 14: search
  `gh pr list --repo juspay/hyperswitch-prism --state all --search "{connector} webhook"`. If a PR exists,
  rebase + re-verify + update it instead of duplicating.
- Push to origin; `gh pr create --repo juspay/hyperswitch-prism --base main --head {BRANCH} --label "GRACE"`.
- PR body MUST include BOTH proofs (creds redacted): the `PRIMARY_EVIDENCE` (ParseEvent + HandleEvent
  grpcurl + responses, incl. the negative tests) and the `SHADOW_EVIDENCE` (the `"Webhook shadow diff"`
  log with `event_type_match=true`/`source_verified` parity, and `bodyComparison.keyDiff: {}` from `:9000`),
  plus the x-request-id used. State explicitly: "hyperswitch Direct gateway used as source of truth;
  validated in shadow (diff empty) then primary (UCS served the webhook live)."

### Phase 8 — Report (return to orchestrator)
```
CONNECTOR: {connector}
STATUS: SUCCESS | FAILED | SKIPPED
CLASSIFICATION: fresh | gap-fill | proto-gap
PRIMARY: PASS | FAIL  (event families covered: ...)
SHADOW: keyDiff {} | <diff signature>   (families covered: ...)
PR: <url or "not created">
REASON: <if not SUCCESS>
```
**STATUS = SUCCESS** only if: build passed AND primary grpcurl passed (incl. negatives) AND shadow diff
empty AND primary-through-router confirmed AND PR created. Anything less is FAILED (with evidence of
attempts) or SKIPPED (not integrated / no creds / no webhooks in hyperswitch source).

---

## 6. Strict definition of done (per connector)

- [ ] `cargo build -p grpc-server` clean.
- [ ] ParseEvent: event_type + reference match hyperswitch for every covered family.
- [ ] HandleEvent: `source_verified:true` on valid signature; content/status match hyperswitch mapping.
- [ ] Negative tests: tampered body / wrong secret → `source_verified:false` (no error, no false-positive).
- [ ] Shadow diff empty (`event_type_match=true`, `source_verified` parity, `bodyComparison.keyDiff: {}`)
      for every covered family.
- [ ] Primary-mode through the router confirmed (UCS serves the live webhook, correct ack).
- [ ] No creds/secrets in the diff; local config files unstaged.
- [ ] PR on `juspay/hyperswitch-prism` with both primary + shadow evidence.

---

## 7. Gotchas (webhook-specific + shadow stack)

1. **Byte-exact message construction is where ~all signature bugs hide.** Match hyperswitch's
   `get_webhook_source_verification_message` exactly — delimiters, timestamp prefix, field order, raw vs
   re-serialized body. Never mutate body bytes before HMAC.
2. **Encoding mismatch** (hex vs base64) silently fails verification. Decode before comparing; some
   connectors (Adyen) compare base64 strings, most (Bluesnap/Ppro/Auth.net) hex-decode first.
3. **Shadow `source_verified` parity needs the MCA webhook secret configured** — otherwise prism can't
   verify and you get a false diff that is a setup issue, not a code bug (Phase 6 step 2).
4. **Webhooks are stateless in the rollout decision** (`should_call_unified_connector_service_for_webhooks`,
   flow name `Webhooks`, `previous_gateway=None`). The rollout key is
   `ucs_rollout_config_<mid>_<conn-or-mca>_Webhooks`.
5. **Shadow compares the parsed outcome, not an outbound request** (no mitm request_diff for inbound
   webhooks). The diff is `WebhookShadowSnapshot` {event_type, source_verified, content_kind, reference}.
6. **Proto event-type gap → two-repo change.** If hyperswitch maps to an event prism's proto
   `WebhookEventType` lacks, follow shadow.md §3.D (branch proto from the pinned tag, rev-pin all
   connector-service deps, thread the field) — heavier; flag it rather than silently dropping the event.
7. **Out-of-band connectors (PayPal/TrueLayer)**: the secret is a webhook-id, not an HMAC key; verification
   is an HTTP round-trip. Follow pattern_verify_webhook_source.md §Family 2 (register in
   `connectors_with_webhook_source_verification_call`, remove from the default-impl macro).
8. **`RUST_MIN_STACK=16777216`** when running a second router instance to capture a full-stack error
   behind an HE_00 (shadow.md §4.4). First UCS call after a prism restart may `Cancelled` — re-fire
   (shadow.md §4.10).
9. **Sequential only** — shared `:8000` and shared diff store; never two connectors at once (shadow.md §4.1).
10. **Don't confuse prism checkouts** — build/run `~/new-grpc/hyperswitch-prism`, never the idle
    `~/hyperswitch-prism/` worktree (shadow.md §4.15).

---

## 8. Reference index

| What | Where |
|---|---|
| Shadow stack topology / setup / gotchas | `~/hyperswitch-prism/hyperswitch/shadow.md` (§1–§4) |
| Source-of-truth trait (hyperswitch) | `crates/hyperswitch_interfaces/src/webhooks.rs` |
| Source-of-truth connector impls | `crates/hyperswitch_connectors/src/connectors/{connector}.rs` |
| Event enum (hyperswitch) | `crates/api_models/src/webhooks.rs` (`IncomingWebhookEvent`) |
| Shadow webhook gateway (Direct + spawn shadow + diff) | `crates/router/src/core/webhooks/gateway.rs` |
| Webhook rollout decision | `crates/router/src/core/unified_connector_service.rs` (`should_call_unified_connector_service_for_webhooks`) |
| Router incoming-webhook route | `POST /webhooks/{merchant_id}/{connector_id_or_name}` (`crates/router/src/routes/app.rs`) |
| Target trait (prism) | `crates/types-traits/interfaces/src/connector_types.rs` (`IncomingWebhook`) |
| Target connector impls (prism) | `crates/integrations/connector-integration/src/connectors/{connector}.rs` |
| Webhook gRPC surface (prism) | `crates/types-traits/grpc-api-types/proto/{payment,services}.proto` (`EventService` ParseEvent/HandleEvent) |
| gRPC server webhook dispatch (prism) | `crates/grpc-server/grpc-server/src/server/events.rs` |
| Connector registry (prism) | `crates/integrations/connector-integration/src/types.rs` (`convert_connector`) |
| Default verify impl macro (prism) | `crates/integrations/connector-integration/src/default_implementations.rs` |
| Grace webhook patterns | `grace/rulesbook/codegen/guides/patterns/pattern_IncomingWebhook_flow.md`, `pattern_verify_webhook_source.md` |
| PR phase reference | `grace/workflow/2.4_pr.md` |
```
