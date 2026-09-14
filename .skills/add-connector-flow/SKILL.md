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

Names in the **Flow** column are the marker structs in
`crates/types-traits/domain_types/src/connector_flow.rs` -- the same spelling the
macros take. Check any name you are unsure of with
`rg -w <Name> crates/types-traits/domain_types/src/connector_flow.rs`.

| Flow | Prerequisites | Why |
|------|--------------|-----|
| Authorize | none | Foundation. |
| PSync | Authorize | `PaymentService_Get/suite_spec.json` threads Authorize's `connector_transaction_id` into the sync request. |
| Capture | Authorize, under **manual** capture | `PaymentService_Capture/suite_spec.json` depends on Authorize scenario `no3ds_manual_capture_credit_card`. |
| Void | Authorize, under **manual** capture | Void cancels an *uncaptured* authorization, so the payment must not have been captured. Suite depends on `no3ds_manual_capture_credit_card`. |
| Refund | Authorize. **Capture too, but only if the connector is on manual capture** | Code-level the prerequisite is Authorize: `RefundsData.connector_transaction_id` is a non-`Option` String holding the *Authorize* id, and checkout/razorpay/cybersource/adyen all build `payments/{connector_transaction_id}/refunds`. Semantically the payment must be *captured*, which auto-capture Authorize satisfies alone. Do not write a flat "Refund needs Capture". |
| RSync | Refund | `RefundService_Get/suite_spec.json` maps `res.connector_refund_id` from the Refund suite. |
| VoidPC | Authorize + Capture | Reverses an already-captured payment; `PaymentService_Reverse/suite_spec.json` is the only suite that names Capture. |
| SetupMandate | none (`CreateConnectorCustomer` in practice) | **Not Authorize.** `PaymentService_SetupRecurring/suite_spec.json` depends only on the token + `CustomerService/Create` suites. SetupMandate is a zero/low-amount card-on-file setup, a sibling of Authorize. |
| RepeatPayment | SetupMandate | Consumes the `connector_mandate_id` SetupMandate returns. |
| MandateRevoke | SetupMandate | `RecurringPaymentService_Revoke/suite_spec.json` depends on `PaymentService/SetupRecurring`. |
| IncrementalAuthorization | Authorize, under **manual** capture | Suite depends on Authorize scenario `no3ds_manual_capture_incremental_auth`. |
| IncomingWebhook | none proven | `EventService_HandleEvent/suite_spec.json` `depends_on` is `[]`. In practice add it after Authorize, since a webhook reports status for a payment that already exists. `IncomingWebhook` is a plain trait, not a `ConnectorIntegrationV2` flow, so nothing forces an order at compile time. |
| ServerAuthenticationToken | none | `depends_on` is `[]`. Gated by `should_do_access_token`, independent of the session-token gate. |
| ServerSessionAuthenticationToken | none | `depends_on` is `[]`. Gated by `should_do_session_token`; the connectors overriding it and those overriding `should_do_access_token` are disjoint sets. |
| ClientAuthenticationToken | none | `depends_on` is `[]`. |
| CreateOrder | none | `PaymentService_CreateOrder/suite_spec.json` `depends_on` is `[]`. |
| CreateConnectorCustomer | none | `CustomerService_Create/suite_spec.json` `depends_on` is `[]`. |
| PaymentMethodToken | CreateConnectorCustomer | **Not Authorize -- the arrow points the other way.** `PaymentMethodService_Tokenize/suite_spec.json` depends on `CustomerService/Create`; Capture/Get/Refund/Void all list Tokenize as *their* dependency, and `PaymentMethodTokenizationData` has no `connector_transaction_id` field to consume. |
| Accept (`FlowName::AcceptDispute`) | none (unproven) | No dispute suite_spec exists; inferred from the request type, which carries a dispute id, not a payment id. |
| SubmitEvidence | none (unproven) | Same -- no dispute suite_spec. |
| DefendDispute | none (unproven) | Same -- no dispute suite_spec. |

Rows marked *(unproven)* have no `suite_spec.json` under
`crates/internal/integration-tests/src/global_suites/`; they are inferred from request
type shape and rpc placement. Do not harden them into rules.

Full dependency graph and resolution algorithm: `references/flow-dependencies.md`

## Critical Conventions

Include in every subagent prompt:

- Use `RouterDataV2` (NEVER `RouterData`), `ConnectorIntegrationV2` (NEVER `ConnectorIntegration`)
- Import from `domain_types` (NEVER `hyperswitch_domain_models`)
- NEVER hardcode status values -- always map via `From`/`TryFrom`
- Use macros for all flows. Every flow in BOTH `create_all_prerequisites!` and `macro_connector_implementation!`
- `generic_type: T` always present in `macro_connector_implementation!` for ALL flows
- The flow you are adding is currently stubbed by `macro_connector_flow_status_impls!`.
  Remove its marker name from that macro's `not_implemented: [...]` list **before** writing
  anything else -- see Step 2 below
- Auth via `req.connector_config` (NOT `connector_auth_type`, deleted from `RouterDataV2`).
  Real signature at `crates/types-traits/interfaces/src/api.rs:25`; copy the idiom from
  `connectors/travelhub.rs`
- `build_error_response` takes THREE parameters (`res`, `Option<&mut events::Event>`,
  `&ConnectorSpecificConfig`) -- `interfaces/src/api.rs:50`. Same third parameter on
  `get_error_response_v2` and `get_5xx_error_response`. There is no `ConnectorEvent` type
  and no `set_error_response_body` method
- `ConnectorError` has exactly FIVE variants (`domain_types/src/errors.rs:371`), all
  requiring a `context` field: `ResponseDeserializationFailed`, `ResponseHandlingFailed`,
  `UnexpectedResponseError`, `IntegrityCheckFailed`, `ConnectorErrorResponse`. Request-side
  failures use `IntegrationError` instead
- `ErrorResponse` implements `Default`, so use `..Default::default()`. `attempt_status` is
  `Option<FlowStatus>`, not `Option<AttemptStatus>` -- and must not be forced terminal on
  the shared error path (`connectors/flywire.rs:362-370`)
- `PaymentsResponseData::TransactionResponse` (11 fields) / `RefundsResponseData` (4 fields)
  have no functional-update shortcut -- list every field or get E0063
- `#[serde(other)] Unknown` on the wire status enum; no catch-all `_ =>` in the
  status-mapping match. Both halves, always
- Error code/message fallbacks: `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` from
  `crates/common/common_utils/src/consts.rs`, never `.unwrap_or_default()`
- Amount unit comes from the vendor spec, not a default. Five types in
  `crates/common/common_utils/src/types.rs`: `MinorUnit`, `StringMinorUnit`,
  `StringMajorUnit`, `FloatMajorUnit`, `StringTwoDecimalUnit`
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
3. **Removes the flow's marker name from `not_implemented: [...]` in the
   `macro_connector_flow_status_impls!` invocation.** That macro
   (`connectors/macros.rs` ~:1827) emits BOTH the marker-trait impl and a stub
   `ConnectorIntegrationV2` impl for every flow it names, so leaving the name in place
   while adding your own is a double **E0119**. Real invocation to copy:
   `connectors/travelhub.rs:455`. Its argument keys are `connector:`, `generic_type:`,
   `[<trait bounds>]`, `not_implemented: [...]`, `not_supported: [...]` (either list may be
   omitted). Skip this step for `IncomingWebhook`: it is a plain trait, not a
   `ConnectorIntegrationV2` flow, so it never appears in that macro
4. Adds flow to `create_all_prerequisites!`
5. Adds `macro_connector_implementation!` block
6. Creates transformer types + TryFrom impls
7. Adds the trait marker implementation, now freed by step 3 (table below)
8. Runs `cargo build --package connector-integration`

Two sibling macros in the same file, for the cases where the standard pair does not fit:
`macro_connector_local_flow_implementation!` (~:2425) for flows with no outbound HTTP call,
and `macro_connector_payout_implementation!` (~:1448) for payout stubs. Read each macro's
first matcher arm before inventing argument keys.

**Flow type quick reference** (full table in `references/flow-implementation-guide.md`):

| Flow | FlowData | RequestData | ResponseData | T? |
|------|----------|-------------|--------------|-----|
| Authorize | PaymentFlowData | PaymentsAuthorizeData\<T\> | PaymentsResponseData | Yes |
| PSync | PaymentFlowData | PaymentsSyncData | PaymentsResponseData | No |
| Capture | PaymentFlowData | PaymentsCaptureData | PaymentsResponseData | No |
| Void | PaymentFlowData | PaymentVoidData | PaymentsResponseData | No |
| Refund | RefundFlowData | RefundsData | RefundsResponseData | No |
| RSync | RefundFlowData | RefundSyncData | RefundsResponseData | No |

**Trait marker names** (not uniform -- use exact names). The **Flow** column is the
marker struct from `connector_flow.rs` (what the macros take); the **Trait** column is
the trait declared in `crates/types-traits/interfaces/src/connector_types.rs`:

| Flow (marker) | Trait |
|------|-------|
| Authorize | `PaymentAuthorizeV2<T>` |
| PSync | `PaymentSyncV2` |
| Capture | `PaymentCapture` |
| Void | `PaymentVoidV2` |
| VoidPC | `PaymentVoidPostCaptureV2` |
| Refund | `RefundV2` |
| RSync | `RefundSyncV2` |
| SetupMandate | `SetupMandateV2<T>` |
| RepeatPayment | `RepeatPaymentV2<T>` |
| MandateRevoke | `MandateRevokeV2` |
| PaymentMethodToken | `PaymentTokenV2<T>` |
| ServerAuthenticationToken | `ServerAuthentication` |
| ServerSessionAuthenticationToken | `ServerSessionAuthentication` |
| ClientAuthenticationToken | `ClientAuthentication` |
| CreateOrder | `PaymentOrderCreate` |
| CreateConnectorCustomer | `CreateConnectorCustomer` |
| IncomingWebhook | `IncomingWebhook` + `SourceVerification` + `BodyDecoding` |
| Accept | `AcceptDispute` |
| SubmitEvidence | `SubmitEvidenceV2` |
| DefendDispute | `DisputeDefend` |

Traps in that table, all of them real and all of them costly to rediscover:

- The dispute-accept marker is `Accept`; `AcceptDispute` is the *trait* (and separately a
  `FlowName` variant). They are not interchangeable.
- `IncomingWebhook` is both a `FlowName` variant and a trait name, but there is **no**
  `IncomingWebhook` marker struct -- it is not a `ConnectorIntegrationV2` flow.
  `SourceVerification` lives in `interfaces/src/verification.rs`, `BodyDecoding` in
  `interfaces/src/decode.rs`; only `IncomingWebhook` is in `connector_types.rs`.
- The three token flows take **`MerchantAuthenticationFlowData`**, not `PaymentFlowData`,
  as their `ConnectorIntegrationV2` ResourceCommonData -- e.g.
  `ConnectorIntegrationV2<connector_flow::ServerAuthenticationToken, MerchantAuthenticationFlowData, ServerAuthenticationTokenRequestData, ServerAuthenticationTokenResponseData>`.
- There is no `CreateAccessToken`, `CreateSessionToken`, `PaymentAccessToken`,
  `PaymentSessionToken`, `AccessTokenRequestData` or `AccessTokenResponseData` anywhere in
  `crates/`. `AccessToken` alone *is* real, but it is a proto value object
  (`message AccessToken` in `proto/payment.proto`), not a flow.

### IncomingWebhook method signatures

Copy these verbatim from `crates/types-traits/interfaces/src/connector_types.rs`; the
arities are the thing people get wrong (**E0061**).

```rust
// ONE argument besides &self.
fn get_event_type(
    &self,
    _request: RequestDetails,
) -> Result<EventType, error_stack::Report<WebhookError>>;

// FOUR arguments besides &self.
fn process_payment_webhook(
    &self,
    _request: RequestDetails,
    _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
    _connector_account_details: Option<ConnectorSpecificConfig>,
    _event_context: Option<domain_types::connector_types::EventContext>,
) -> Result<WebhookDetailsResponse, error_stack::Report<WebhookError>>;

fn get_webhook_event_reference(
    &self,
    _request: RequestDetails,
) -> Result<Option<WebhookResourceReference>, error_stack::Report<WebhookError>>;

fn get_webhook_integrity_checks(&self) -> Vec<WebhookIntegrityCheck>;
```

The error type is `WebhookError`, not `IntegrationError`. There is no
`transformation_status` field and no `WebhookTransformationStatus` type anywhere in
`crates/` -- referencing either is **E0560**.

`SourceVerification` and `BodyDecoding` are NON-generic traits: exactly ONE impl per
connector, never one per flow. `impl<T> SourceVerification<Flow, Data, Req, Resp> for X<T>`
is **E0107**. Exemplar: `connectors/travelhub.rs:175-183`.

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
| VoidPC | `types.PaymentService/Reverse` |
| MandateRevoke | `types.RecurringPaymentService/Revoke` |
| CreateOrder | `types.PaymentService/CreateOrder` |
| CreateConnectorCustomer | `types.CustomerService/Create` |
| PaymentMethodToken | `types.PaymentMethodService/Tokenize` |
| ServerAuthenticationToken | `types.MerchantAuthenticationService/CreateServerAuthenticationToken` |
| ServerSessionAuthenticationToken | `types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken` |
| ClientAuthenticationToken | `types.MerchantAuthenticationService/CreateClientAuthenticationToken` |
| IncomingWebhook | `types.EventService/HandleEvent` |

The proto package is `types` for every service file except `health_check.proto`, whose
`package` statement is `grpc.health.v1`. Read the `package` statement itself rather than
any header comment. Never address an rpc as `ucs.v2.*` -- there is no such package.

**Gate:** All new flows must pass before proceeding.

---

### Step 4: Quality Review (MANDATORY subagent)

> **CRITICAL: You MUST delegate quality review to a subagent. Do NOT review yourself.**
> **Subagent prompt:** `references/subagent-prompts.md` → Subagent 4
> **Checklist:** `references/quality-checklist.md`

**What it does:**
1. Architecture compliance (no legacy types)
2. Status mapping: no hardcoded statuses outside match arms; `#[serde(other)] Unknown` on the
   wire enum AND no `_ =>` in the mapping match
3. Code quality (no unwrap, descriptive errors, `NO_ERROR_CODE`/`NO_ERROR_MESSAGE` rather
   than `.unwrap_or_default()`, no blanket terminal `attempt_status`)
4. Macro completeness: the new flow is in both macros, its marker name is GONE from
   `macro_connector_flow_status_impls!`'s `not_implemented` list, and its trait marker impl
   appears exactly once (a duplicate is E0119)
5. Consistency with existing flows in this connector
6. Final `cargo build`

---

## Supported Flows Catalog

| Category | Flows |
|----------|-------|
| Core | Authorize, PSync, Capture, Void, VoidPC, Refund, RSync |
| Pre-Auth | ServerAuthenticationToken, ServerSessionAuthenticationToken, ClientAuthenticationToken, CreateOrder, CreateConnectorCustomer, PaymentMethodToken |
| Mandate/Recurring | SetupMandate, RepeatPayment, MandateRevoke |
| Dispute | Accept, SubmitEvidence, DefendDispute |
| Webhook | IncomingWebhook (requires SourceVerification + BodyDecoding traits) |
| Auth | PreAuthenticate, Authenticate, PostAuthenticate |

The authoritative catalog is the 48 `pub struct` markers in
`crates/types-traits/domain_types/src/connector_flow.rs`; this table is the subset this
skill has patterns for. `connector_flow.rs` also defines a `FlowName` enum whose variant
set is *not* identical to the marker set -- `IncomingWebhook` and `Dsync` (one capital)
have no marker, while `Accept`, `PSync`, `RSync`, `VoidPC` and `VerifyWebhookSource` have
no identically-spelled `FlowName` variant (`FlowName` spells the first four
`AcceptDispute`, `Psync`, `Rsync`, `VoidPc`). The macros take the marker spelling.

---

## Reference Index

| Path | Contents |
|------|----------|
| `references/subagent-prompts.md` | Full prompts for all 4 subagents |
| `references/flow-implementation-guide.md` | 3-part procedure, flow type table, per-flow subagent prompt |
| `references/grpc-testing-guide.md` | gRPC service map, grpcurl templates, test validation criteria |
| `references/flow-dependencies.md` | Dependency graph, capture-model conditionals, resolution algorithm, existing-flow detection |
| `references/macro-reference.md` | Both core macros, parameters, content types, generic rules |
| `references/type-system.md` | Core imports, RouterDataV2, domain_types structure |
| `references/quality-checklist.md` | Pre-submission quality gates |
| `references/utility-functions.md` | Error handling, amount conversion helpers |
| `references/flow-patterns/*.md` | Per-flow: authorize, psync, capture, refund, rsync, void |
