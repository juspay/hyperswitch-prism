---
name: new-connector
description: >
  Implements a new payment connector from scratch in the connector-service (UCS) Rust codebase.
  Creates connector foundation and implements all 6 core payment flows (Authorize, PSync, Capture,
  Refund, RSync, Void). Use when integrating a new payment gateway that does not yet exist.
  Requires a technical specification at grace/rulesbook/codegen/references/{connector_name}/technical_specification.md.
license: Apache-2.0
compatibility: Requires Rust toolchain with cargo. Linux or macOS.
metadata:
  author: parallal
  version: "2.0"
  domain: payment-connectors
---

# New Connector Implementation

## Overview

This skill produces a complete payment connector in the UCS Rust codebase.

**MANDATORY SUBAGENT DELEGATION: You are the orchestrator. You MUST delegate every step
to a subagent using the prompts in `references/subagent-prompts.md`. Do NOT implement
code, run tests, or review quality yourself. Spawn subagents and coordinate their outputs.**

**Output:**
- Main connector file with macro-based flow implementations
- Transformers module with request/response types and conversions
- Registration in the connector registry
- All 6 core flows + any required pre-auth flows
- gRPC tested end-to-end
- Certification manifest (`connector_specs/{connector_name}/specs.json`) that survives the
  merge-blocking CI gate — see Step 6

**Prerequisites:**
- Tech spec at `grace/rulesbook/codegen/references/{connector_name}/technical_specification.md`
- Rust toolchain with `cargo`

## Project Structure

| Purpose | Path |
|---------|------|
| Main connector file | `crates/integrations/connector-integration/src/connectors/{connector_name}.rs` |
| Transformers module | `crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs` |
| Connector registry | `crates/integrations/connector-integration/src/connectors.rs` |
| Enum definitions | `crates/common/common_enums/src/enums.rs` |
| Domain utilities | `crates/types-traits/domain_types/src/utils.rs` |
| Macro definitions | `crates/integrations/connector-integration/src/connectors/macros.rs` |
| Superposition URLs | `config/superposition.toml` |
| URL patching (`Connectors::apply`) | `crates/types-traits/domain_types/src/types.rs` |

## Critical Conventions

These rules apply to ALL subagents. Include them in every subagent prompt.

- Use `RouterDataV2` (NEVER `RouterData`), `ConnectorIntegrationV2` (NEVER `ConnectorIntegration`)
- Import from `domain_types` (NEVER `hyperswitch_domain_models`)
- Connector struct MUST be generic: `ConnectorName<T>`
- NEVER hardcode status values -- always map from connector response via `From`/`TryFrom`
- Use macros (`create_all_prerequisites!` + `macro_connector_implementation!`) for all flows
- Flows you do NOT implement are stubbed by `macro_connector_flow_status_impls!`
  (`not_implemented: [...]` / `not_supported: [...]`) -- see "The macro nobody remembers" below
- Check `references/utility-functions.md` before implementing custom helpers
- No `unwrap()`, no fields hardcoded to `None`, no unnecessary `.clone()`
- Auth data accessed via `req.connector_config` (NOT `connector_auth_type`, which was
  deleted from `RouterDataV2`). `get_auth_header` takes `&ConnectorSpecificConfig`
  (`crates/types-traits/interfaces/src/api.rs:25`); copy the idiom from a recent connector
  such as `connectors/travelhub.rs`
- `build_error_response` takes THREE parameters --
  `(res: Response, _event_builder: Option<&mut events::Event>, _connector_config: &ConnectorSpecificConfig)`
  (`interfaces/src/api.rs:50`). The event type is `events::Event`; there is no `ConnectorEvent`
  in this crate, and `events::Event` has no `set_error_response_body` method. The same
  third parameter applies to `get_error_response_v2` and `get_5xx_error_response`
- `ErrorResponse` has 13 fields and DOES implement `Default`
  (`domain_types/src/router_data.rs`), so prefer `..Default::default()` over listing them.
  `attempt_status` is `Option<FlowStatus>`, not `Option<AttemptStatus>` -- wrap it, e.g.
  `Some(FlowStatus::Payment(AttemptStatus::Failure))`, and never force a terminal status on
  the shared error path (exemplar: `connectors/flywire.rs:362-370`; minimal form
  `connectors/noon.rs:499-512`)
- `PaymentsResponseData::TransactionResponse` (11 fields) and `RefundsResponseData`
  (4 fields) are read from `domain_types/src/connector_types.rs`. Enum struct-variants have
  no functional-update syntax, so every omitted field is E0063 -- list them all
- Error codes/messages: `.unwrap_or_else(|| NO_ERROR_CODE.to_string())` and
  `NO_ERROR_MESSAGE`, from `crates/common/common_utils/src/consts.rs`. Never
  `error_code.unwrap_or_default()`
- Amount unit: read the vendor spec and match its wire format. The five types in
  `crates/common/common_utils/src/types.rs` are `MinorUnit`, `StringMinorUnit`,
  `StringMajorUnit`, `FloatMajorUnit`, `StringTwoDecimalUnit`. There is no safe default --
  `rg -o "amount_type: \w+" crates/integrations/connector-integration/src/connectors/*.rs`
  at HEAD splits ~10/10/9/4 across StringMajorUnit / MinorUnit / StringMinorUnit / FloatMajorUnit
- Status enums need `#[serde(other)] Unknown` at the DESERIALIZATION layer; the
  status-mapping `match` must stay exhaustive over named variants (no `_ =>` there).
  Reviewers require both halves
- An in-band failure returned with HTTP 2xx must produce `Err(ErrorResponse { .. })`,
  branching on a success predicate -- see `utils::is_payment_failure` in
  `crates/types-traits/domain_types/src/utils.rs`
- Connector base URLs MUST be registered in `config/superposition.toml` (dimension enum + sandbox &
  production `connector_base_url` overrides) AND the connector wired into
  `Connectors::patch_connector_urls` in `crates/types-traits/domain_types/src/types.rs` for dynamic
  URL patching. The scaffold script (`add_connector.sh`) now does this automatically — verify it
  landed, and pass `--production-url` when the live URL differs from the sandbox base URL.

---

## Workflow: Orchestrator Sequence

Each step below is an independent subagent. The orchestrator delegates each step,
waits for completion, and passes outputs to the next step.

**Full subagent prompts:** `references/subagent-prompts.md`

### Step 1: Tech Spec Validation (Subagent)

> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 1

**Inputs:** connector_name

**What it does:**
- Reads the tech spec
- Extracts: name, base_url, auth method, amount format, content type
- Lists all supported flows with HTTP methods and endpoints
- Detects pre-auth flows:

| Pre-Auth Flow (marker in `connector_flow.rs`) | Detect when... |
|---------------|---------------|
| ServerAuthenticationToken | OAuth/token auth (POST /login, /oauth/token) |
| CreateOrder | Order/intent required before payment |
| CreateConnectorCustomer | Customer object required before payment |
| PaymentMethodToken | Tokenization required before authorize |
| ServerSessionAuthenticationToken | Session init required before payment |

> Marker names are the structs in `crates/types-traits/domain_types/src/connector_flow.rs`.
> There is no `CreateAccessToken` / `CreateSessionToken` / `PaymentAccessToken` /
> `PaymentSessionToken` anywhere in `crates/` — use the names above verbatim.

**Outputs:** connector config, list of flows, list of pre-auth flows

**Gate (HARD STOP — no exceptions):**
If tech spec missing → **STOP IMMEDIATELY. Do NOT proceed to Step 2.**
Tell the user: "No tech spec found for {ConnectorName}. Please either:
(1) Run the `generate-tech-spec` skill first, or
(2) Provide the tech spec file manually at `grace/rulesbook/codegen/references/{connector_name}/technical_specification.md`."
Do NOT attempt to infer API details from any other source. A tech spec is mandatory.

---

### Step 2: Foundation Setup (Subagent)

> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 2

**Inputs:** connector_name, base_url, production_base_url (optional), auth_method, amount_type (from Step 1)

**What it does:**
- Runs `.skills/new-connector/scripts/add_connector.sh {connector_name} {base_url} --force -y`
  (that path is a symlink to the real script, `grace/rulesbook/codegen/add_connector.sh`; either
  path works, there is no `scripts/add_connector.sh` at the repo root)
  - add `--production-url {production_base_url}` when the live URL differs from the sandbox base URL
  - add `--flows {Flow1},{Flow2},...` only when this connector needs suites beyond the six core
    flows; see "specs.json" below
- Verifies `cargo build --package connector-integration` passes
- Checks UCS conventions (RouterDataV2, generic struct, domain_types imports)
- Sets up `create_amount_converter_wrapper!` macro
- Implements `ConnectorCommon` trait (id, content_type, base_url, auth_header, error_response)
- **VERIFIES** the base trait markers the scaffold already emitted -- it does NOT add them.
  `add_connector.sh` writes `ConnectorServiceTrait<T>`, `ValidationTrait`, `IncomingWebhook`,
  `VerifyRedirectResponse` and `SourceVerification` impls, and `connector.rs.template`
  writes `BodyDecoding`. Writing any of them a second time is a conflicting implementation
  (**E0119**), not a no-op. `SourceVerification` and `BodyDecoding` are NON-generic traits
  (`interfaces/src/verification.rs`, `interfaces/src/decode.rs`): one impl per connector,
  never one per flow -- a per-flow `impl<T> SourceVerification<Flow, Data, Req, Resp>` is
  **E0107**. Working exemplar: `connectors/travelhub.rs:175`
- The scaffold script auto-registers the connector's base URLs in `config/superposition.toml`
  (dimension enum + sandbox/production overrides) and adds the URL-patching arm in `types.rs`
  `Connectors::patch_connector_urls` — the subagent verifies both landed
- The scaffold script also writes
  `crates/internal/integration-tests/src/connector_specs/{connector_name}/specs.json`, which the
  ungated CI check `cargo run --all-features --bin check_connector_specs` requires. It seeds
  `supported_suites` from `--flows`, defaulting to the six core flows
  (`Authorize,PSync,Capture,Void,Refund,RSync`), and merges into an existing file rather than
  overwriting it. Pass `--flows` only to seed a different set, e.g.
  `--flows Authorize,PSync,Capture,Void,Refund,RSync,SetupMandate`.
  `--flows` takes `check_connector_specs` flow names, NOT the trait names `--list-flows` prints —
  the accepted vocabulary is the `flow_to_suite` table in the script (Authorize, PSync, Capture,
  Void, Refund, RSync, SetupMandate, RepeatPayment, MandateRevoke, CreateConnectorCustomer,
  GetConnectorCustomer, PaymentMethodToken, PaymentMethodEligibility, ServerAuthenticationToken,
  ClientAuthenticationToken, ServerSessionAuthenticationToken, PreAuthenticate, Authenticate,
  PostAuthenticate, CreateOrder, IncrementalAuthorization). An unrecognised name aborts the run.

**Outputs:** scaffold created, superposition URLs registered + URL patching wired,
`crates/internal/integration-tests/src/connector_specs/{connector_name}/specs.json` written, build passing, files list

**Gate:** Build must pass before proceeding.

---

### Step 3: Flow Implementation (MANDATORY subagent per flow, sequential)

> **CRITICAL: You MUST delegate each flow to a subagent. Do NOT implement code yourself.**
> Read the subagent prompt from `references/subagent-prompts.md` → Subagent 3, fill in the
> variables ({ConnectorName}, {FlowName}, tech spec path), and spawn a subagent for EACH flow.
> Wait for each subagent to complete before spawning the next.

> **Detailed procedure:** `references/flow-implementation-guide.md`
> **Per-flow patterns:** `references/flow-patterns/{flow}.md`
> **Macro reference:** `references/macro-reference.md`

**Execution order** (strict sequential — spawn one subagent per flow, wait for completion):

1. Pre-auth flows (only if detected in Step 1):
   ServerAuthenticationToken → ServerSessionAuthenticationToken → CreateOrder →
   CreateConnectorCustomer → PaymentMethodToken

   Ordering notes: `ServerAuthenticationToken` and `ServerSessionAuthenticationToken` have NO
   prerequisite (both suite specs under
   `crates/internal/integration-tests/src/global_suites/MerchantAuthenticationService_*` have an
   empty `depends_on`), so they go first and neither requires the other. `PaymentMethodToken`
   depends on `CreateConnectorCustomer` — NOT on Authorize; tokenization runs before Authorize,
   not after it.

2. Core flows (always):
   Authorize → PSync → Capture → Refund → RSync → Void

**Each flow subagent does:**
1. Reads tech spec for this flow's endpoint details
2. Reads `references/flow-patterns/{flow}.md` for patterns
3. **Removes the flow's marker name from the `not_implemented: [...]` list in the
   `macro_connector_flow_status_impls!` invocation.** Do this FIRST. That macro emits both
   the marker-trait impl and a stub `ConnectorIntegrationV2` impl for every flow it lists,
   so leaving the name there while adding your own is a double **E0119**
4. Adds flow to `create_all_prerequisites!` with correct types
5. Adds `macro_connector_implementation!` block
6. Creates request/response types + TryFrom impls in transformers.rs
7. Adds the flow's trait marker implementation -- now that step 3 freed it (marker names
   are not uniform; the table is in `references/flow-implementation-guide.md`)
8. Runs `cargo build --package connector-integration`
9. Reports SUCCESS or FAILED

### The macro nobody remembers

`crates/integrations/connector-integration/src/connectors/macros.rs` defines three macros
beyond the two above. All 111 connector files at HEAD invoke the first one.

| Macro | Where | What it takes |
|-------|-------|---------------|
| `macro_connector_flow_status_impls!` | `macros.rs` ~:1827 | `connector:`, `generic_type:`, `[<bounds>]`, `not_implemented: [...]`, `not_supported: [...]` (either list may be omitted) |
| `macro_connector_local_flow_implementation!` | `macros.rs` ~:2425 | flows with no outbound HTTP call |
| `macro_connector_payout_implementation!` | `macros.rs` ~:1448 | `connector:`, `generic_type:`, `[<bounds>]` -- payout flow stubs |

A real invocation to copy (`connectors/travelhub.rs:455`):

```rust
macros::macro_connector_flow_status_impls!(
    connector: Travelhub,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept, ClientAuthenticationToken, CreateConnectorCustomer, GetConnectorCustomer,
        DefendDispute, MandateRevoke, Authenticate, IncrementalAuthorization, CreateOrder,
        PostAuthenticate, PreAuthenticate, PaymentMethodToken, VoidPC, RepeatPayment,
        ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
```

Read the macro's first matcher arm before inventing argument keys.

**Key type reference** (full table in `references/flow-implementation-guide.md`):

| Flow | FlowData | RequestData | ResponseData | T? |
|------|----------|-------------|--------------|-----|
| Authorize | PaymentFlowData | PaymentsAuthorizeData\<T\> | PaymentsResponseData | Yes |
| PSync | PaymentFlowData | PaymentsSyncData | PaymentsResponseData | No |
| Capture | PaymentFlowData | PaymentsCaptureData | PaymentsResponseData | No |
| Void | PaymentFlowData | PaymentVoidData | PaymentsResponseData | No |
| Refund | RefundFlowData | RefundsData | RefundsResponseData | No |
| RSync | RefundFlowData | RefundSyncData | RefundsResponseData | No |

---

### Step 4: gRPC Testing (MANDATORY subagent)

> **CRITICAL: You MUST delegate testing to a subagent. Do NOT run grpcurl yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 4
> **Full testing guide:** `references/grpc-testing-guide.md`

**Inputs:** connector_name, list of implemented flows, creds.json

**What it does:**
1. Starts gRPC server (`cargo run --bin grpc-server`)
2. Loads credentials from `creds.json`
3. Tests each flow via grpcurl against the correct service/method
4. Validates: status 2xx, no errors, correct status value
5. If test fails: reads server logs, fixes code, rebuilds, retests

**Key gRPC service mapping** (full table in testing guide):

| Flow | gRPC Method |
|------|-------------|
| Authorize | `types.PaymentService/Authorize` |
| PSync | `types.PaymentService/Get` |
| Capture | `types.PaymentService/Capture` |
| Void | `types.PaymentService/Void` |
| Refund | `types.PaymentService/Refund` |
| RSync | `types.RefundService/Get` |

**Anti-loop safeguards:** 3-strike rule, max 7 iterations, must change code between retries.

**Gate:** All flows must pass before proceeding.

---

### Step 5: Quality Review (MANDATORY subagent)

> **CRITICAL: You MUST delegate quality review to a subagent. Do NOT review yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 5
> **Checklist:** `references/quality-checklist.md`

**What it does:**
1. Architecture compliance: no RouterData (non-V2), no hyperswitch_domain_models
2. Status mapping: no hardcoded statuses outside match arms; `#[serde(other)] Unknown` on the
   wire enum AND no `_ =>` in the status-mapping match
3. Code quality: no unwrap(), no None-hardcoded fields, descriptive errors,
   `NO_ERROR_CODE`/`NO_ERROR_MESSAGE` instead of `unwrap_or_default()`
4. Macro completeness: every implemented flow in both `create_all_prerequisites!` and
   `macro_connector_implementation!`, with its marker name REMOVED from
   `macro_connector_flow_status_impls!`'s `not_implemented` list; every unimplemented flow
   still listed there; no duplicate base-trait impls (E0119)
5. Error types: only the five real `ConnectorError` variants
   (`ResponseDeserializationFailed`, `ResponseHandlingFailed`, `UnexpectedResponseError`,
   `IntegrityCheckFailed`, `ConnectorErrorResponse`) — the first four carry a
   `context: ResponseTransformationErrorContext`, while `ConnectorErrorResponse` wraps a
   `Box<ErrorResponse>`; request-side failures use `IntegrationError`
6. Naming conventions: {ConnectorName}{Flow}Request/Response pattern
7. Final build: `cargo build --package connector-integration`

**Outputs:** PASS with 0 violations, or FAIL with list of violations to fix.

The subagent runs the **Pre-Flight Gate** (`references/quality-checklist.md` §15) and
reports its output verbatim, not a summary. Each of the six items has a mechanical check;
"reviewed and looks fine" is a FAIL. The one most often waved through is §15.5 — *PSync
does not re-derive a lookup key it could have read*. The naive form of that check ("does
PSync read the carrier?") passes on a read that ends in `unwrap_or(..)` or `.ok()`, which
is exactly the bug: it syncs the wrong transaction the moment the carrier is absent. A
missing carrier must be `IntegrationError::MissingRequiredField`.

---

### Step 6: Certification (MANDATORY — orchestrator-run; this is where "done" lives)

No subagent prompt exists for this step; the orchestrator runs it after Step 5 reports PASS.

> **Full reference:** `.skills/_shared/references/certification.md`
> (`references/` has no symlink for this file yet — use the `_shared` path.)

**`cargo build` is step 1 of 8, not the finish line.** Since 2026-08-31 (`75079740f`)
connector certification is **merge-blocking**. A branch that compiles, lints clean and
answers grpcurl by hand can still be unmergeable, and the three mechanisms that block it
fail for different reasons:

| Mechanism | Scope | Escape hatch |
|---|---|---|
| `cargo run --bin check_connector_specs` (Compilation Check job) | every connector, every run | none |
| `.github/scripts/verify-new-connectors.sh` (Run Tests job) | connectors **added by this PR** | `alpha_connectors.json` + a `reason` |
| `.github/scripts/certify-connectors.sh` (Run Tests job) | connectors this PR **touched** | merge-base arbitration |

A connector counts as **new** when `connector_specs/<name>/` did not exist at the PR's base
commit. Splitting the PR does not change that. Every connector this skill produces is new.

**Definition of done** — the orchestrator does not report success until all eight hold:

1. `cargo build --package connector-integration` — zero errors, zero warnings
2. `cargo run --bin check_connector_specs` prints `All checks passed. OK.`
   *(Run it locally. Phase 1 exits 1 when a file under `connectors/` has no matching
   `connector_specs/<name>/` directory, so a scaffold without specs.json cannot merge.)*
3. `specs.json` **trimmed to what is actually implemented**. It is a certification claim,
   not a scaffold artifact: a suite declared but not implemented fails against the sandbox;
   a flow implemented but not declared fails `check_connector_specs` Phase 2. If it declares
   `EventService/HandleEvent`, a real captured `webhook_payload.json` sits next to it
   (Phase 2b) — do not fabricate one
4. Certification decided **in writing**: either the connector has CI credentials and every
   declared scenario passes
   `./scripts/run-tests --connector <name> --interface grpc --report`, or an
   `alpha_connectors.json` entry with a **specific, non-empty `reason`**. A bare `{}` entry
   is `exit 1` for a new connector. Listing it posts a public "merging without live sandbox
   proof" comment on the PR; removing the name later is a *promotion* that pulls the
   connector into the full sweep, so `add_connector.sh` deliberately never edits that file —
   add the entry by hand
5. Proto ordinal re-checked against `origin/main` after the last commit (someone may have
   merged an enum value first — renumber if the max is `>=` yours)
6. Superposition URLs + `Connectors::patch_connector_urls` verified
   (`cargo test -p grpc-server --all-features --test test_superposition_config`)
7. Pre-Flight Gate clean (`references/quality-checklist.md` §15)
8. Evidence regenerated **against the branch head** after the last fix commit. Captures
   from an earlier revision are evidence for code that is not being merged

**PR body requirements** (`references/quality-checklist.md` §16). GRACE does not write
tests — that stands — so the PR body is the only place a human learns what needs one. Any
**novel algorithmic logic** in the diff (signing/HMAC, hashing, checksum, custom amount
encoding, a timestamp or nonce format that feeds a signature, bespoke serialization) is
**listed line by line with the spec section it implements and a concrete input → expected
output**, so a reviewer can add the known-answer test. Also state the certification status
and paste the `check_connector_specs` output.

**"Out of scope" is not "do not build it."** `add_connector.sh`'s `is_out_of_scope_flow()`
lists flows excluded from *certification*. The Rust source it mirrors
(`OUT_OF_SCOPE_FLOWS` in `crates/internal/integration-tests/src/bin/check_connector_specs.rs`)
says why: *"Each is a coverage gap, not a decision that it should never be covered."*
`VoidPC` is on that list and is implemented by 14 connectors. Implement what the tech spec
calls for; it simply contributes no suite to `supported_suites`.

**Fork PRs skip certification entirely** (the Run Tests job's `RUN_TESTS` is false when
`head.repo != base.repo`). A green fork PR is not a certified connector — it is certified in
the merge queue, where `merge_group` turns the gate back on.

---

## Reference Index

| Path | Contents |
|------|----------|
| `references/subagent-prompts.md` | Full copy-paste prompts for all 5 subagents |
| `references/flow-implementation-guide.md` | 3-part flow procedure, type table (17 flows), per-flow subagent prompt |
| `references/grpc-testing-guide.md` | gRPC service map, grpcurl templates, test validation, testing subagent prompt |
| `references/macro-reference.md` | Both core macros, parameters, content types, generic rules |
| `references/type-system.md` | Core imports, type paths, domain_types module structure |
| `references/utility-functions.md` | Error handling, card formatting, amount conversion helpers |
| `references/quality-checklist.md` | Pre-submission checklist, §15 Pre-Flight Gate, §16 PR-body disclosure, common mistakes |
| `.skills/_shared/references/certification.md` | The merge-blocking certification gate: `check_connector_specs` phases, `verify-new-connectors.sh` fail paths, `alpha_connectors.json`, definition of done |
| `references/flow-patterns/*.md` | Per-flow: authorize, psync, capture, refund, rsync, void |
| `.skills/new-connector/scripts/add_connector.sh` | Scaffold script that generates initial connector files (symlink to `grace/rulesbook/codegen/add_connector.sh`) |
