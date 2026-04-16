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
| Macro definitions | `crates/integrations/connector-integration/src/connectors/macros/` |

## Critical Conventions

These rules apply to ALL subagents. Include them in every subagent prompt.

- Use `RouterDataV2` (NEVER `RouterData`), `ConnectorIntegrationV2` (NEVER `ConnectorIntegration`)
- Import from `domain_types` (NEVER `hyperswitch_domain_models`)
- Connector struct MUST be generic: `ConnectorName<T>`
- NEVER hardcode status values -- always map from connector response via `From`/`TryFrom`
- Use macros (`create_all_prerequisites!` + `macro_connector_implementation!`) for all flows
- Check `references/utility-functions.md` before implementing custom helpers
- No `unwrap()`, no fields hardcoded to `None`, no unnecessary `.clone()`
- Auth data accessed via `req.connector_config` (NOT `connector_auth_type`)

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

| Pre-Auth Flow | Detect when... |
|---------------|---------------|
| CreateAccessToken | OAuth/token auth (POST /login, /oauth/token) |
| CreateOrder | Order/intent required before payment |
| CreateConnectorCustomer | Customer object required before payment |
| PaymentMethodToken | Tokenization required before authorize |
| CreateSessionToken | Session init required before payment |

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

**Inputs:** connector_name, base_url, auth_method, amount_type (from Step 1)

**What it does:**
- Runs `scripts/add_connector.sh {connector_name} {base_url} --force -y`
- Verifies `cargo build --package connector-integration` passes
- Checks UCS conventions (RouterDataV2, generic struct, domain_types imports)
- Sets up `create_amount_converter_wrapper!` macro
- Implements `ConnectorCommon` trait (id, content_type, base_url, auth_header, error_response)
- Adds required trait markers (ConnectorServiceTrait, SourceVerification, BodyDecoding)

**Outputs:** scaffold created, build passing, files list

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
   CreateAccessToken → CreateOrder → CreateConnectorCustomer → PaymentMethodToken → CreateSessionToken

2. Core flows (always):
   Authorize → PSync → Capture → Refund → RSync → Void

**Each flow subagent does:**
1. Reads tech spec for this flow's endpoint details
2. Reads `references/flow-patterns/{flow}.md` for patterns
3. Adds flow to `create_all_prerequisites!` with correct types
4. Adds `macro_connector_implementation!` block
5. Creates request/response types + TryFrom impls in transformers.rs
6. Adds trait marker implementation
7. Runs `cargo build --package connector-integration`
8. Reports SUCCESS or FAILED

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
2. Status mapping: no hardcoded statuses outside match arms
3. Code quality: no unwrap(), no None-hardcoded fields, descriptive errors
4. Macro completeness: every flow in both macros + trait markers
5. Naming conventions: {ConnectorName}{Flow}Request/Response pattern
6. Final build: `cargo build --package connector-integration`

**Outputs:** PASS with 0 violations, or FAIL with list of violations to fix.

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
| `references/quality-checklist.md` | Pre-submission checklist, common mistakes |
| `references/flow-patterns/*.md` | Per-flow: authorize, psync, capture, refund, rsync, void |
| `scripts/add_connector.sh` | Scaffold script that generates initial connector files |
