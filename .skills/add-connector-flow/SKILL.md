---
name: add-connector-flow
description: >
  Adds one or more payment flows (Authorize, Capture, Refund, Void, PSync, RSync, webhooks, etc.)
  to an existing connector in the connector-service (UCS) Rust codebase. Use when a connector
  already exists but is missing specific flow implementations. Handles dependency validation
  and sequential implementation order.
license: Apache-2.0
compatibility: Requires Rust toolchain with cargo. Linux or macOS.
metadata:
  author: parallal
  version: "2.0"
  domain: payment-connectors
---

# Add Connector Flow

## Overview

Adds specific payment flows to an existing connector.

**MANDATORY SUBAGENT DELEGATION: You are the orchestrator. You MUST delegate every step
to a subagent using the prompts in `references/subagent-prompts.md`. Do NOT implement
code, run tests, or review quality yourself. Spawn subagents and coordinate their outputs.**

**Inputs:** connector name + list of flows to add (e.g., "add Refund and RSync to AcmePay")
**Output:** requested flows implemented, tested, and quality-reviewed

## Flow Dependencies

Flows must be implemented in dependency order. A flow cannot be added unless its
prerequisites already exist or are also being added in the same batch.

| Flow | Prerequisites |
|------|--------------|
| Authorize | None (foundation) |
| PSync | Authorize |
| Capture | Authorize |
| Void | Authorize |
| Refund | Authorize |
| RSync | Refund |
| SetupMandate | Authorize |
| RepeatPayment | SetupMandate |
| IncomingWebhook | PSync |
| CreateAccessToken | None |
| CreateOrder | None |
| CreateConnectorCustomer | None |
| PaymentMethodToken | Authorize |
| AcceptDispute | None |
| SubmitEvidence | None |
| DefendDispute | None |

Full dependency graph and resolution algorithm: `references/flow-dependencies.md`

## Critical Conventions

Include in every subagent prompt:

- Use `RouterDataV2` (NEVER `RouterData`), `ConnectorIntegrationV2` (NEVER `ConnectorIntegration`)
- Import from `domain_types` (NEVER `hyperswitch_domain_models`)
- NEVER hardcode status values -- always map via `From`/`TryFrom`
- Use macros for all flows. Every flow in BOTH `create_all_prerequisites!` and `macro_connector_implementation!`
- `generic_type: T` always present in `macro_connector_implementation!` for ALL flows
- Auth via `req.connector_config` (NOT `connector_auth_type`)
- No `unwrap()`, no None-hardcoded fields

---

## Workflow: Orchestrator Sequence

**Full subagent prompts:** `references/subagent-prompts.md`

### Step 1: State Analysis & Dependency Validation (Subagent)

> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 1

**Inputs:** connector_name, requested_flows

**What it does:**
1. Verifies connector exists at expected path
2. Reads connector file, lists flows already in `create_all_prerequisites!`
3. Reads tech spec for each requested flow's API details
4. Validates dependencies -- checks each requested flow's prerequisites exist
5. Resolves implementation order (topological sort)

**Outputs:** existing_flows, implementation_order, missing_prerequisites

**Gates (HARD STOP — no exceptions):**
- If tech spec missing → **STOP IMMEDIATELY. Do NOT proceed to Step 2.**
  Tell the user: "No tech spec found for {ConnectorName}. Please either:
  (1) Run the `generate-tech-spec` skill first, or
  (2) Provide the tech spec file manually at `grace/rulesbook/codegen/references/{connector_name}/technical_specification.md`."
  Do NOT attempt to infer API details from existing connector code or any other source.
  A tech spec is mandatory — never skip this requirement.
- If prerequisites missing → STOP, inform user what to add first.

---

### Step 2: Flow Implementation (MANDATORY subagent per flow, sequential)

> **CRITICAL: You MUST delegate each flow to a subagent. Do NOT implement code yourself.**
> Read the subagent prompt from `references/subagent-prompts.md` → Subagent 2, fill in the
> variables ({ConnectorName}, {FlowName}, tech spec path), and spawn a subagent for EACH flow.
> Wait for each subagent to complete before spawning the next.

> **Detailed procedure:** `references/flow-implementation-guide.md`
> **Per-flow patterns:** `references/flow-patterns/{flow}.md`

Implement each flow in the resolved order from Step 1. **Spawn one subagent per flow.**

**Each flow subagent does:**
1. Reads tech spec for this flow's endpoint
2. Reads `references/flow-patterns/{flow}.md`
3. Adds flow to `create_all_prerequisites!`
4. Adds `macro_connector_implementation!` block
5. Creates transformer types + TryFrom impls
6. Adds trait marker implementation
7. Runs `cargo build --package connector-integration`

**Flow type quick reference** (full table in `references/flow-implementation-guide.md`):

| Flow | FlowData | RequestData | ResponseData | T? |
|------|----------|-------------|--------------|-----|
| Authorize | PaymentFlowData | PaymentsAuthorizeData\<T\> | PaymentsResponseData | Yes |
| PSync | PaymentFlowData | PaymentsSyncData | PaymentsResponseData | No |
| Capture | PaymentFlowData | PaymentsCaptureData | PaymentsResponseData | No |
| Void | PaymentFlowData | PaymentVoidData | PaymentsResponseData | No |
| Refund | RefundFlowData | RefundsData | RefundsResponseData | No |
| RSync | RefundFlowData | RefundSyncData | RefundsResponseData | No |

**Trait marker names** (not uniform -- use exact names):

| Flow | Trait |
|------|-------|
| Authorize | `PaymentAuthorizeV2<T>` |
| PSync | `PaymentSyncV2` |
| Capture | `PaymentCapture` |
| Void | `PaymentVoidV2` |
| Refund | `RefundV2` |
| RSync | `RefundSyncV2` |
| SetupMandate | `SetupMandateV2<T>` |
| RepeatPayment | `RepeatPaymentV2<T>` |
| PaymentMethodToken | `PaymentTokenV2<T>` |
| CreateAccessToken | `PaymentAccessToken` |
| CreateOrder | `PaymentOrderCreate` |
| CreateSessionToken | `PaymentSessionToken` |
| CreateConnectorCustomer | `CreateConnectorCustomer` |
| IncomingWebhook | `IncomingWebhook` + `SourceVerification` + `BodyDecoding` |
| AcceptDispute | `AcceptDispute` |
| SubmitEvidence | `SubmitEvidenceV2` |
| DefendDispute | `DisputeDefend` |

---

### Step 3: gRPC Testing (MANDATORY subagent)

> **CRITICAL: You MUST delegate testing to a subagent. Do NOT run grpcurl yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 3
> **Testing guide:** `references/grpc-testing-guide.md`

**Inputs:** connector_name, list of newly added flows

**What it does:**
1. Starts gRPC server
2. Loads credentials from `creds.json`
3. Tests each new flow via grpcurl
4. Validates responses (PASS/FAIL criteria in testing guide)
5. If failed: reads server logs, fixes code, rebuilds, retests (max 7 iterations)

**gRPC service mapping** (full table in testing guide):

| Flow | gRPC Method |
|------|-------------|
| Authorize | `types.PaymentService/Authorize` |
| PSync | `types.PaymentService/Get` |
| Capture | `types.PaymentService/Capture` |
| Void | `types.PaymentService/Void` |
| Refund | `types.PaymentService/Refund` |
| RSync | `types.RefundService/Get` |
| SetupMandate | `types.PaymentService/SetupRecurring` |
| RepeatPayment | `types.RecurringPaymentService/Charge` |

**Gate:** All new flows must pass before proceeding.

---

### Step 4: Quality Review (MANDATORY subagent)

> **CRITICAL: You MUST delegate quality review to a subagent. Do NOT review yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 4
> **Checklist:** `references/quality-checklist.md`

**What it does:**
1. Architecture compliance (no legacy types)
2. Status mapping (no hardcoded statuses)
3. Code quality (no unwrap, descriptive errors)
4. Consistency with existing flows in this connector
5. Final `cargo build`

---

## Supported Flows Catalog

| Category | Flows |
|----------|-------|
| Core | Authorize, PSync, Capture, Void, Refund, RSync |
| Pre-Auth | CreateAccessToken, CreateOrder, CreateConnectorCustomer, PaymentMethodToken, CreateSessionToken |
| Mandate/Recurring | SetupMandate, RepeatPayment, MandateRevoke |
| Dispute | AcceptDispute, SubmitEvidence, DefendDispute |
| Webhook | IncomingWebhook (requires SourceVerification + BodyDecoding traits) |
| Auth | PreAuthenticate, Authenticate, PostAuthenticate |

---

## Reference Index

| Path | Contents |
|------|----------|
| `references/subagent-prompts.md` | Full prompts for all 4 subagents |
| `references/flow-implementation-guide.md` | 3-part procedure, type table (17 flows), per-flow subagent prompt |
| `references/grpc-testing-guide.md` | gRPC service map, grpcurl templates, test validation criteria |
| `references/flow-dependencies.md` | Dependency graph and resolution algorithm |
| `references/macro-reference.md` | Both core macros, parameters, content types, generic rules |
| `references/type-system.md` | Core imports, RouterDataV2, domain_types structure |
| `references/quality-checklist.md` | Pre-submission quality gates |
| `references/utility-functions.md` | Error handling, amount conversion helpers |
| `references/flow-patterns/*.md` | Per-flow: authorize, psync, capture, refund, rsync, void |
