# Flow Implementation Guide

This is the step-by-step procedure for implementing a single flow in a UCS connector.
Each flow follows the same 4-part pattern: add to prerequisites macro, add implementation
macro, create transformer types, and **de-register the flow's stub**. Then build and fix.

Part 0 below is the step most guides omit, and skipping it produces a conflicting-implementation
error that looks nothing like the mistake that caused it.

Read `macro-reference.md` before using this guide.

---

## Part 0: Remove the Flow from the Stub Macro

Every connector carries a `macro_connector_flow_status_impls!` invocation listing the flows it does
**not** implement (112 of 112 connectors at HEAD have one). It generates a
`ConnectorIntegrationV2` impl for each listed flow. If you implement a flow that is still listed
there, you get two impls of the same trait for the same types:

```
error[E0119]: conflicting implementations of trait `ConnectorIntegrationV2<Capture, ...>`
```

So before anything else, open the connector file and **delete the flow you are about to implement**
from the `not_implemented:` (or `not_supported:`) list:

```rust
macros::macro_connector_flow_status_impls!(
    connector: ExamplePay,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        // Capture,          <-- delete this line when implementing Capture
        DefendDispute,
        SetupMandate,
        SubmitEvidence
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
```

The reverse also holds: if you decide **not** to implement a flow, it must be added to one of those
two lists, or the connector fails to compile with `E0277: the trait bound ... is not satisfied`.
See `macro-reference.md` for the full argument contract and the `not_implemented` vs
`not_supported` distinction.

---

## Part 1: Add Flow to create_all_prerequisites!

Add a flow entry to the `api` array in `create_all_prerequisites!`:

```rust
(
    flow: {FlowName},
    request_body: {ConnectorName}{FlowName}Request,  // omit for GET endpoints
    response_body: {ConnectorName}{FlowName}Response,
    router_data: RouterDataV2<{FlowName}, {FlowData}, {RequestData}, {ResponseData}>,
),
```

### Flow Type Reference Table

| Flow | FlowData | RequestData | ResponseData | Generic T? |
|------|----------|-------------|--------------|------------|
| Authorize | PaymentFlowData | PaymentsAuthorizeData\<T\> | PaymentsResponseData | Yes |
| PSync | PaymentFlowData | PaymentsSyncData | PaymentsResponseData | No |
| Capture | PaymentFlowData | PaymentsCaptureData | PaymentsResponseData | No |
| Void | PaymentFlowData | PaymentVoidData | PaymentsResponseData | No |
| Refund | RefundFlowData | RefundsData | RefundsResponseData | No |
| RSync | RefundFlowData | RefundSyncData | RefundsResponseData | No |
| SetupMandate | PaymentFlowData | SetupMandateRequestData\<T\> | PaymentsResponseData | Yes |
| RepeatPayment | PaymentFlowData | RepeatPaymentData\<T\> | PaymentsResponseData | Yes |
| ServerAuthenticationToken | MerchantAuthenticationFlowData | ServerAuthenticationTokenRequestData | ServerAuthenticationTokenResponseData | No |
| CreateOrder | PaymentFlowData | PaymentCreateOrderData | PaymentCreateOrderResponse | No |
| CreateConnectorCustomer | PaymentFlowData | ConnectorCustomerData | ConnectorCustomerResponse | No |
| PaymentMethodToken | PaymentFlowData | PaymentMethodTokenizationData\<T\> | PaymentMethodTokenResponse | Yes |
| ServerSessionAuthenticationToken | MerchantAuthenticationFlowData | ServerSessionAuthenticationTokenRequestData | ServerSessionAuthenticationTokenResponseData | No |
| ClientAuthenticationToken | MerchantAuthenticationFlowData | ClientAuthenticationTokenRequestData | PaymentsResponseData | No |
| IncrementalAuthorization | PaymentFlowData | PaymentsIncrementalAuthorizationData | PaymentsResponseData | No |
| Accept | DisputeFlowData | AcceptDisputeData | DisputeResponseData | No |
| SubmitEvidence | DisputeFlowData | SubmitEvidenceData | DisputeResponseData | No |
| DefendDispute | DisputeFlowData | DisputeDefendData | DisputeResponseData | No |

**Rules:**
- For Authorize, SetupMandate, RepeatPayment, PaymentMethodToken: request_body includes `<T>`
- For PSync, RSync (GET endpoints): omit `request_body` entirely
- Every flow must appear in BOTH macros. A flow in only one macro will not compile.
- The flow name in the table is the **marker struct** from
  `crates/types-traits/domain_types/src/connector_flow.rs`. There is no `CreateAccessToken`,
  `CreateSessionToken`, `AccessTokenRequestData` or `SessionTokenRequestData` in this codebase --
  the access/session token family is `ServerAuthenticationToken` / `ServerSessionAuthenticationToken` /
  `ClientAuthenticationToken`, and all three use `MerchantAuthenticationFlowData`, **not**
  `PaymentFlowData`. Verify any flow name with
  `rg -w <Name> crates/types-traits/domain_types/src/connector_flow.rs` before using it.

---

## Part 2: Add macro_connector_implementation! Block

For each flow, add a `macro_connector_implementation!` invocation:

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}{FlowName}Request),  // omit for GET
    curl_response: {ConnectorName}{FlowName}Response,
    flow_name: {FlowName},
    resource_common_data: {FlowData},       // see the Flow Type Reference Table above
    flow_request: {RequestData},
    flow_response: {ResponseData},
    http_method: Post,                       // Post, Get, Put, Delete
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<{FlowName}, {FlowData}, {RequestData}, {ResponseData}>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<{FlowName}, {FlowData}, {RequestData}, {ResponseData}>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/endpoint", self.connector_base_url_payments(req)))
        }
    }
);
```

### Key Rules

- `generic_type: T` is ALWAYS present for ALL flows, even those that don't use `<T>` on request types.
- `curl_request` never includes `<T>`. Write `Json(ConnRequest)` not `Json(ConnRequest<T>)`.
- Omit `curl_request` entirely for GET endpoints (PSync, RSync).
- Use `connector_base_url_payments` for payment flows, `connector_base_url_refunds` for refund flows.
- `resource_common_data` must match column 2 of the table: `PaymentFlowData` for payment flows,
  `RefundFlowData` for Refund/RSync, `DisputeFlowData` for the dispute flows, and
  `MerchantAuthenticationFlowData` for the three token flows.
- URL construction for flows operating on existing transactions must extract the transaction ID
  from the request data and interpolate it into the URL path.

---

## Part 3: Create Transformer Types

In `transformers.rs`, define for each flow:

### Request Type (Serialize)

```rust
#[derive(Debug, Serialize)]
pub struct {ConnectorName}{FlowName}Request {
    // Fields matching the connector API specification
}
```

### Response Type (Deserialize)

```rust
#[derive(Debug, Deserialize)]
pub struct {ConnectorName}{FlowName}Response {
    pub status: {ConnectorName}PaymentStatus,  // or RefundStatus
    pub id: String,
    // Other fields from the connector API response
}
```

### Status Enum (Deserialize) with From impl

```rust
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]  // or SCREAMING_SNAKE_CASE, camelCase
pub enum {ConnectorName}PaymentStatus {
    Success,
    Pending,
    Failed,
    // All status values from the connector API
    #[serde(other)]
    Unknown,
}

impl From<{ConnectorName}PaymentStatus> for AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            {ConnectorName}PaymentStatus::Success => Self::Charged,
            {ConnectorName}PaymentStatus::Pending => Self::Pending,
            {ConnectorName}PaymentStatus::Failed => Self::Failure,
            // Non-terminal: a later PSync can still resolve it.
            {ConnectorName}PaymentStatus::Unknown => Self::Pending,
        }
    }
}
```

**CRITICAL**: Never hardcode status. Always map from the connector response via From/TryFrom.

**Both halves of the unknown-status rule are required**, and reviewers check for both:

- `#[serde(other)] Unknown` at the **deserialization** layer. Without it, a status string the
  vendor adds later makes the whole response fail to deserialize -- a real charge then surfaces as
  a transport error. 76 status enums at HEAD carry this attribute.
- **No `_ =>` catch-all** at the **status-mapping** layer. Match every variant explicitly, so the
  compiler tells you when a variant is added instead of silently funnelling it into whatever the
  catch-all says.

Use per-flow terminal states, not the generic `Failure`: a failed Capture is
`AttemptStatus::CaptureFailed`, a failed Void is `VoidFailed`, a declined authorization is
`AuthorizationFailed`. Refund flows map to `RefundStatus`, never `AttemptStatus`.

### TryFrom for Request (RouterDataV2 → connector request)

```rust
impl<T: PaymentMethodDataTypes> TryFrom<&{ConnectorName}RouterData<&RouterDataV2<
    Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData
>, T>> for {ConnectorName}PaymentRequest<T> {
    type Error = Report<errors::IntegrationError>;
    fn try_from(item: &{ConnectorName}RouterData<&RouterDataV2<...>, T>) -> Result<Self, Self::Error> {
        // Extract fields from item.router_data.request
        // Use item.amount for converted amount
    }
}
```

### TryFrom for Response (connector response → domain response)

`ResponseRouterData` takes exactly **two** type parameters -- the connector response type and the
router data type -- and carries three fields: `response`, `router_data`, `http_code`
(`crates/integrations/connector-integration/src/types.rs:345`). The idiomatic form uses `Self` for
the second parameter:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<ResponseRouterData<{ConnectorName}PaymentResponse, Self>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
{
    type Error = error_stack::Report<ConnectorError>;
    fn try_from(
        item: ResponseRouterData<{ConnectorName}PaymentResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let status = AttemptStatus::from(item.response.status);

        // A 2xx body can still say "declined". Branch on the failure predicate and
        // return the Err side explicitly -- otherwise a declined payment is reported
        // as authorized.
        let response = if utils::is_payment_failure(status) {
            Err(ErrorResponse {
                code: item
                    .response
                    .error_code
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                message: item
                    .response
                    .error_message
                    .clone()
                    .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                reason: item.response.error_message.clone(),
                status_code: item.http_code,
                attempt_status: Some(FlowStatus::Payment(status)),
                connector_transaction_id: Some(item.response.id.clone()),
                ..Default::default()
            })
        } else {
            Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                redirection_data: None,
                connector_metadata: None,
                mandate_reference: None,
                network_txn_id: None,
                network_txn_link_id: None,
                connector_response_reference_id: None,
                incremental_authorization_allowed: None,
                splits: None,
                status_code: item.http_code,
                payment_account_reference: None,
            })
        };

        Ok(Self {
            // `status` lives on PaymentFlowData, NOT on RouterDataV2 itself.
            resource_common_data: PaymentFlowData {
                status,
                ..item.router_data.resource_common_data
            },
            response,
            ..item.router_data
        })
    }
}
```

**Gotchas in this block:**
- `RouterDataV2` has no `status` field. Its fields are `flow`, `resource_common_data`,
  `connector_config`, `request`, `response` (`domain_types/src/router_data_v2.rs:6`).
  Payment status is `resource_common_data.status`.
- `PaymentsResponseData::TransactionResponse` is an **enum variant**, so `..Default::default()`
  is not valid there -- every one of its 11 fields must be named. (`RefundsResponseData` is a plain
  struct with four fields -- `connector_refund_id: String`, `refund_status`, `status_code: u16`,
  `acquirer_reference_number: Option<String>` -- and does not derive `Default` either.)
- The response direction returns `error_stack::Report<ConnectorError>`; the request direction
  (above) returns `error_stack::Report<IntegrationError>`.
- **In-band 2xx failure must return `Err(ErrorResponse{..})`**, as shown. The predicates are
  `domain_types::utils::is_payment_failure` and
  `connector_integration::utils::is_refund_failure`; do not re-derive which statuses count as
  failure.
- `ErrorResponse` has **13** fields, but it has a hand-written `impl Default`
  (`domain_types/src/router_data.rs:4244`), so `..Default::default()` covers the trailing
  `network_*` and `typed_/raw_connector_*` fields.
- `attempt_status` is `Option<FlowStatus>`, **not** `Option<AttemptStatus>`. `FlowStatus` is
  domain-tagged (`Payment` / `Refund` / `Dispute` / `Payout`), so a refund flow emits
  `FlowStatus::Refund(RefundStatus::Failure)`, never a `Payment` variant. On a *shared* error path
  (`build_error_response`), classify the error first and default to a **non-terminal** status --
  see `connectors/noon.rs:499-512` for the minimal form and `connectors/flywire.rs:362-370` for the
  refund-vs-payment form. A blanket `Some(AttemptStatus::Failure)` reports charged payments as
  FAILURE; a blanket `None` leaves hard-declined refunds Pending and retrying forever.
- `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` come from `common_utils::consts`. Never
  `.unwrap_or_default()` an error code -- an empty string is indistinguishable from a real one.

See the flow-specific pattern file (`flow-patterns/{flow}.md`) for complete examples
tailored to each flow (payment vs refund vs dispute types, GET vs POST, etc.).

---

## Part 4: Build and Fix

After each flow:

```bash
cargo build --package connector-integration
```

Common errors and fixes:
- Missing imports in transformers.rs → add the required `use` statement
- Type mismatches between macro parameters and transformer types → check the type table above
- Wrong `resource_common_data` → PaymentFlowData for payments, RefundFlowData for refunds,
  DisputeFlowData for disputes, MerchantAuthenticationFlowData for the token flows
- Missing `<T>` on Authorize/SetupMandate request types
- Incorrect `From` impl target → AttemptStatus for payments, RefundStatus for refunds
- `E0119 conflicting implementations` → the flow is still listed in
  `macro_connector_flow_status_impls!`; see Part 0
- `E0277 the trait bound ... ConnectorIntegrationV2<...> is not satisfied` → a flow marker is in
  neither macro; add it to `not_implemented:` or `not_supported:`
- `E0107 trait takes 0 generic arguments but 4 were supplied` → `SourceVerification` and
  `BodyDecoding` are **non-generic**. Write one `impl SourceVerification for {Conn}<T> {}` per
  connector, not one per flow (exemplar `connectors/travelhub.rs:175`)
- `E0063 missing field` in a `PaymentsResponseData::TransactionResponse` literal → enum
  struct-variants have no functional-update syntax; name all 11 fields
- `E0599 no variant named ...` on an error enum → read the live variant lists in
  `crates/types-traits/domain_types/src/errors.rs`. `ConnectorError` has exactly five variants and
  none of them are `InvalidData`, `NotImplemented` or `InvalidCard`
- `E0609 no field connector_auth_type` → auth now comes from
  `req.connector_config: ConnectorSpecificConfig`
- `E0050 method has 2 parameters but the declaration has 3` on `build_error_response` /
  `get_error_response_v2` / `get_5xx_error_response` → add the third
  `_connector_config: &ConnectorSpecificConfig` parameter

---

## Subagent Prompt Template

Use this prompt to delegate a single flow to a subagent:

```
Implement the {FlowName} flow for the {ConnectorName} connector in the UCS codebase.

## Context
- Tech spec: grace/rulesbook/codegen/references/{connector}/technical_specification.md
- Connector file: crates/integrations/connector-integration/src/connectors/{connector}.rs
- Transformers: crates/integrations/connector-integration/src/connectors/{connector}/transformers.rs

## Instructions
1. Read the tech spec to understand the {FlowName} endpoint (URL, method, request/response schema, status values)
2. Read the flow pattern: .skills/new-connector/references/flow-patterns/{flow}.md
3. Read the implementation guide: .skills/new-connector/references/flow-implementation-guide.md
4. Implement:
   a. Remove {FlowName} from the not_implemented/not_supported list in
      macro_connector_flow_status_impls! (otherwise E0119 conflicting implementations)
   b. Add flow entry to create_all_prerequisites! macro in the connector file
   c. Add macro_connector_implementation! block
   d. Create request/response types and TryFrom impls in transformers.rs
   e. Add the trait marker implementation if not already present
5. Run: cargo build --package connector-integration
6. Fix any compilation errors
7. Self-check against .skills/_shared/references/quality-checklist.md sections 3, 9 and 10:
   - status enum has #[serde(other)] Unknown, and the From impl has no `_ =>` arm
   - no .unwrap_or_default() on an error code/message (use NO_ERROR_CODE / NO_ERROR_MESSAGE)
   - in-band 2xx failures return Err(ErrorResponse{..}), gated on is_payment_failure /
     is_refund_failure
   - attempt_status is Option<FlowStatus>, domain-correct, and non-terminal by default
   - the amount unit matches the vendor spec's wire format (do not default to StringMinorUnit)
8. Report SUCCESS or FAILED with details
```
