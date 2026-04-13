# Flow Implementation Guide

This is the step-by-step procedure for implementing a single flow in a UCS connector.
Each flow follows the same 3-part pattern: add to prerequisites macro, add implementation
macro, create transformer types. Then build and fix.

Read `macro-reference.md` before using this guide.

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
| CreateAccessToken | PaymentFlowData | AccessTokenRequestData | AccessTokenResponseData | No |
| CreateOrder | PaymentFlowData | PaymentCreateOrderData | PaymentCreateOrderResponse | No |
| CreateConnectorCustomer | PaymentFlowData | ConnectorCustomerData | ConnectorCustomerResponse | No |
| PaymentMethodToken | PaymentFlowData | PaymentMethodTokenizationData\<T\> | PaymentMethodTokenResponse | Yes |
| CreateSessionToken | PaymentFlowData | SessionTokenRequestData | SessionTokenResponseData | No |
| IncrementalAuthorization | PaymentFlowData | PaymentsIncrementalAuthorizationData | PaymentsResponseData | No |
| Accept | DisputeFlowData | AcceptDisputeData | DisputeResponseData | No |
| SubmitEvidence | DisputeFlowData | SubmitEvidenceData | DisputeResponseData | No |
| DefendDispute | DisputeFlowData | DisputeDefendData | DisputeResponseData | No |

**Rules:**
- For Authorize, SetupMandate, RepeatPayment, PaymentMethodToken: request_body includes `<T>`
- For PSync, RSync (GET endpoints): omit `request_body` entirely
- Every flow must appear in BOTH macros. A flow in only one macro will not compile.

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
    resource_common_data: {FlowData},       // PaymentFlowData or RefundFlowData
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
}

impl From<{ConnectorName}PaymentStatus> for AttemptStatus {
    fn from(status: {ConnectorName}PaymentStatus) -> Self {
        match status {
            {ConnectorName}PaymentStatus::Success => Self::Charged,
            {ConnectorName}PaymentStatus::Pending => Self::Pending,
            {ConnectorName}PaymentStatus::Failed => Self::Failure,
        }
    }
}
```

**CRITICAL**: Never hardcode status. Always map from the connector response via From/TryFrom.

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

```rust
impl<T> TryFrom<ResponseRouterData<Authorize, {ConnectorName}PaymentResponse, PaymentsAuthorizeData<T>, PaymentsResponseData>>
    for RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData> {
    type Error = Report<errors::IntegrationError>;
    fn try_from(item: ResponseRouterData<...>) -> Result<Self, Self::Error> {
        Ok(Self {
            status: AttemptStatus::from(item.response.status),
            response: Ok(PaymentsResponseData::TransactionResponse {
                resource_id: ResponseId::ConnectorTransactionId(item.response.id),
                ..Default::default()
            }),
            ..item.data
        })
    }
}
```

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
- Wrong `resource_common_data` → PaymentFlowData for payments, RefundFlowData for refunds
- Missing `<T>` on Authorize/SetupMandate request types
- Incorrect `From` impl target → AttemptStatus for payments, RefundStatus for refunds

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
   a. Add flow entry to create_all_prerequisites! macro in the connector file
   b. Add macro_connector_implementation! block
   c. Create request/response types and TryFrom impls in transformers.rs
   d. Add the trait marker implementation if not already present
5. Run: cargo build --package connector-integration
6. Fix any compilation errors
7. Report SUCCESS or FAILED with details
```
