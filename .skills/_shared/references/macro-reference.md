# UCS Macro Reference

## Two Core Macros

All connector implementations use two macros from `super::macros`:

1. **`create_all_prerequisites!`** -- Creates the connector struct, flow bridges, amount converters, and shared helper methods.
2. **`macro_connector_implementation!`** -- Implements `ConnectorIntegrationV2` for a single flow.

Every flow must appear in BOTH macros. A flow defined only in one will fail to compile.

---

## create_all_prerequisites!

```rust
macros::create_all_prerequisites!(
    connector_name: ExamplePay,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ExamplePayPaymentRequest<T>,
            response_body: ExamplePayPaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            // request_body omitted -- GET endpoint, no body
            response_body: ExamplePaySyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: ExamplePayCaptureRequest,
            response_body: ExamplePayCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: ExamplePayVoidRequest,
            response_body: ExamplePayVoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: ExamplePayRefundRequest,
            response_body: ExamplePayRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
        ),
        (
            flow: RSync,
            response_body: ExamplePayRefundResponse,
            router_data: RouterDataV2<RSync, RefundFlowData, RefundSyncData, RefundsResponseData>,
        ),
    ],
    amount_converters: [
        amount_converter: StringMinorUnit
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                "application/json".to_string().into(),
            )];
            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.examplepay.base_url
        }

        pub fn connector_base_url_refunds<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, RefundFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.examplepay.base_url
        }
    }
);
```

### Parameters

| Parameter | Description |
|-----------|-------------|
| `connector_name` | PascalCase struct name (e.g., `Stripe`, `Adyen`) |
| `generic_type` | Always `T` |
| `api` | Array of flow definitions (see flow table below) |
| `amount_converters` | Amount conversion utilities |
| `member_functions` | Shared helpers available to all flows |

### What It Generates

- `pub struct ExamplePay<T> { ... }` -- the connector struct
- `pub struct ExamplePayRouterData<RD, T> { ... }` -- input data wrapper
- Bridge implementations for request/response handling
- Amount converter wrappers

---

## macro_connector_implementation!

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ExamplePay,
    curl_request: Json(ExamplePayPaymentRequest),
    curl_response: ExamplePayPaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
        }
    }
);
```

### Parameters

| Parameter | Description |
|-----------|-------------|
| `connector_default_implementations` | Always `[get_content_type, get_error_response_v2]` |
| `connector` | Connector struct name (must match `create_all_prerequisites!`) |
| `curl_request` | Content type wrapping request type. Omit entirely for GET endpoints |
| `curl_response` | Response type |
| `flow_name` | Must match a flow in `create_all_prerequisites!` |
| `resource_common_data` | `PaymentFlowData`, `RefundFlowData`, or `DisputeFlowData` |
| `flow_request` | Domain request data type |
| `flow_response` | Domain response data type |
| `http_method` | `Post`, `Get`, `Put`, `Patch`, or `Delete` |
| `generic_type` | Always `T` |
| `[trait_bounds]` | Always `[PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]` |
| `other_functions` | Flow-specific `get_headers` and `get_url` implementations |

### What It Generates

- Complete `ConnectorIntegrationV2` trait implementation
- `get_request_body` method (from `curl_request`)
- `handle_response_v2` method (from `curl_response`)
- Default implementations for `get_content_type` and `get_error_response_v2`

---

## Content Type Selection

| curl_request value | When to use |
|--------------------|-------------|
| `Json(RequestType)` | JSON API requests (most connectors) |
| `FormUrlEncoded(RequestType)` | URL-encoded form bodies (legacy APIs) |
| `FormData(RequestType)` | Multipart form uploads (file uploads, evidence) |
| **Omit entirely** | GET requests with no body (sync flows) |

When `curl_request` is omitted, also omit `request_body` in the corresponding `create_all_prerequisites!` flow definition.

---

## resource_common_data Mapping

| resource_common_data | Flows |
|----------------------|-------|
| `PaymentFlowData` | Authorize, PSync, Capture, Void, VoidPC, SetupMandate |
| `RefundFlowData` | Refund, RSync |
| `DisputeFlowData` | Accept, SubmitEvidence, DefendDispute |

This also determines which `connector_base_url_*` helper to use. A `PaymentFlowData` base URL helper has signature `&'a RouterDataV2<F, PaymentFlowData, Req, Res>`, a `RefundFlowData` helper uses `RefundFlowData`, etc.

---

## Generic Type T Rules

**Use `<T>` on request types when** the flow receives payment method data:
- `Authorize` -- `ExamplePayPaymentRequest<T>`, `PaymentsAuthorizeData<T>`
- `SetupMandate` -- `ExamplePayMandateRequest<T>`, `SetupMandateRequestData<T>`

**Do NOT use `<T>` on request types when** the flow operates on an existing transaction:
- `PSync` -- `ExamplePaySyncRequest`, `PaymentsSyncData`
- `Capture` -- `ExamplePayCaptureRequest`, `PaymentsCaptureData`
- `Void` -- `ExamplePayVoidRequest`, `PaymentVoidData`
- `Refund` -- `ExamplePayRefundRequest`, `RefundsData`
- `RSync` -- no request type, `RefundSyncData`

Note: In `curl_request` inside `macro_connector_implementation!`, do NOT include `<T>` even for generic types. Write `Json(ExamplePayPaymentRequest)` not `Json(ExamplePayPaymentRequest<T>)`. The `<T>` appears only in `request_body` inside `create_all_prerequisites!` and in `flow_request`.

---

## Amount Converters

| Type | Use when |
|------|----------|
| `StringMinorUnit` | Connector expects amounts as string minor units (cents). Most common. |
| `FloatMajorUnit` | Connector expects decimal major units (e.g., `12.50`) |
| `StringMajorUnit` | Connector expects string major units |
| `MinorUnit` | Connector expects integer minor units |

Usage in transformers:
```rust
let amount = connector.amount_converter.convert(
    router_data.request.minor_amount,
    router_data.request.currency,
)?;
```

---

## Flow Quick-Reference Table

| Flow | request_body | response_body | flow_request | flow_response | resource_common_data | HTTP |
|------|-------------|---------------|-------------|---------------|---------------------|------|
| Authorize | `ConnPaymentRequest<T>` | `ConnPaymentResponse` | `PaymentsAuthorizeData<T>` | `PaymentsResponseData` | `PaymentFlowData` | Post |
| PSync | omit or `ConnSyncRequest` | `ConnSyncResponse` | `PaymentsSyncData` | `PaymentsResponseData` | `PaymentFlowData` | Get/Post |
| Capture | `ConnCaptureRequest` | `ConnCaptureResponse` | `PaymentsCaptureData` | `PaymentsResponseData` | `PaymentFlowData` | Post |
| Void | `ConnVoidRequest` | `ConnVoidResponse` | `PaymentVoidData` | `PaymentsResponseData` | `PaymentFlowData` | Post |
| VoidPC | `ConnVoidPCRequest` | `ConnVoidPCResponse` | `PaymentsCancelPostCaptureData` | `PaymentsResponseData` | `PaymentFlowData` | Post |
| Refund | `ConnRefundRequest` | `ConnRefundResponse` | `RefundsData` | `RefundsResponseData` | `RefundFlowData` | Post |
| RSync | omit | `ConnRefundResponse` | `RefundSyncData` | `RefundsResponseData` | `RefundFlowData` | Get |
| SetupMandate | `ConnMandateRequest<T>` | `ConnMandateResponse` | `SetupMandateRequestData<T>` | `PaymentsResponseData` | `PaymentFlowData` | Post |
| Accept | `ConnAcceptRequest` | `ConnAcceptResponse` | `AcceptDisputeData` | `DisputeResponseData` | `DisputeFlowData` | Post |
| SubmitEvidence | `ConnEvidenceRequest` | `ConnEvidenceResponse` | `SubmitEvidenceData` | `DisputeResponseData` | `DisputeFlowData` | Post |
| DefendDispute | `ConnDefendRequest` | `ConnDefendResponse` | `DisputeDefendData` | `DisputeResponseData` | `DisputeFlowData` | Post |

(`Conn` is shorthand for the connector name prefix, e.g., `ExamplePay`.)

---

## GET Flow Pattern (No Request Body)

In `create_all_prerequisites!`:
```rust
(
    flow: PSync,
    response_body: ExamplePaySyncResponse,
    router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
),
```

In `macro_connector_implementation!`:
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ExamplePay,
    // curl_request omitted -- no request body
    curl_response: ExamplePaySyncResponse,
    flow_name: PSync,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsSyncData,
    flow_response: PaymentsResponseData,
    http_method: Get,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            let id = req.request.connector_transaction_id.clone();
            Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
        }
    }
);
```

---

## URL Construction Patterns

Static endpoint:
```rust
Ok(format!("{}/v1/payments", self.connector_base_url_payments(req)))
```

With transaction ID (string field):
```rust
let id = req.request.connector_transaction_id.clone();
Ok(format!("{}/v1/payments/{}", self.connector_base_url_payments(req), id))
```

With transaction ID (ResponseId enum, used in Capture):
```rust
let id = match &req.request.connector_transaction_id {
    ResponseId::ConnectorTransactionId(id) => id,
    _ => return Err(errors::IntegrationError::MissingConnectorTransactionID.into())
};
Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
```

---

## Common Mistakes

1. **Wrong resource_common_data**: Using `PaymentFlowData` for Refund/RSync flows. Refund flows use `RefundFlowData`.

2. **Missing flow in prerequisites**: Every `macro_connector_implementation!` flow must have a matching entry in `create_all_prerequisites!` api array.

3. **`<T>` in curl_request**: Write `Json(ConnPaymentRequest)` not `Json(ConnPaymentRequest<T>)`. The generic goes in `request_body` and `flow_request`, not in `curl_request`.

4. **Forgetting to omit curl_request for GET**: When `http_method: Get`, omit `curl_request` entirely. Also omit `request_body` from the prerequisites flow definition.

5. **Base URL helper mismatch**: `connector_base_url_payments` accepts `PaymentFlowData`; `connector_base_url_refunds` accepts `RefundFlowData`. Using the wrong one causes a type error.

---

## Naming Conventions

| Item | Pattern | Example |
|------|---------|---------|
| Payment request | `{Conn}PaymentRequest<T>` | `ExamplePayPaymentRequest<T>` |
| Payment response | `{Conn}PaymentResponse` | `ExamplePayPaymentResponse` |
| Sync request | `{Conn}SyncRequest` | `ExamplePaySyncRequest` |
| Sync response | `{Conn}SyncResponse` | `ExamplePaySyncResponse` |
| Capture request | `{Conn}CaptureRequest` | `ExamplePayCaptureRequest` |
| Void request | `{Conn}VoidRequest` | `ExamplePayVoidRequest` |
| Refund request | `{Conn}RefundRequest` | `ExamplePayRefundRequest` |
| Refund response | `{Conn}RefundResponse` | `ExamplePayRefundResponse` |
| Error response | `{Conn}ErrorResponse` | `ExamplePayErrorResponse` |
| Auth type | `{Conn}AuthType` | `ExamplePayAuthType` |
