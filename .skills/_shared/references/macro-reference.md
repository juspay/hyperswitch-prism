# UCS Macro Reference

## Five Macros That Matter

All connector implementations draw on macros from `super::macros`
(`crates/integrations/connector-integration/src/connectors/macros.rs`). Two build the flows you
actually call out on; three fill in everything else. A connector that declares only the first two
will not compile, because the framework requires a `ConnectorIntegrationV2` impl for *every* flow
marker, not just the ones you implemented.

| Macro | Purpose | Adoption at HEAD |
|-------|---------|------------------|
| **`create_all_prerequisites!`** | Connector struct, flow bridges, amount converters, shared helper methods | every connector |
| **`macro_connector_implementation!`** | `ConnectorIntegrationV2` for a single flow that makes an outbound HTTP call | every connector |
| **`macro_connector_flow_status_impls!`** | Stub impls for flows this connector does not implement or does not support | **112 of 112 connectors** |
| **`macro_connector_payout_implementation!`** | Stub impls for the nine payout flows | most connectors |
| **`macro_connector_local_flow_implementation!`** | A flow with **no** outbound HTTP call -- the response is built locally | rare (`kount.rs`, `worldpayxml.rs`) |

Every flow you implement must appear in BOTH `create_all_prerequisites!` and
`macro_connector_implementation!`. A flow defined in only one will fail to compile. Every flow you
do **not** implement must be listed in `macro_connector_flow_status_impls!` (or, for payouts,
covered by `macro_connector_payout_implementation!`), or you get `E0277: the trait bound
... ConnectorIntegrationV2<...> is not satisfied`.

Before copying any invocation below, read the macro's first matcher arm in `macros.rs` -- the
argument keys are the contract, and they are the thing that drifts.

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
        // Pick the unit the vendor's wire format actually uses -- see "Amount Converters"
        // below. `amount_converters: []` is legal when transformers call `convert_amount`
        // directly.
        amount_converter: StringMajorUnit
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
| `connector_default_implementations` | Usually `[get_content_type, get_error_response_v2]`. Add `get_headers` when the flow needs no flow-specific header logic (33 invocations at HEAD do). Any name listed here is generated by the macro, so it must NOT also appear in `other_functions` |
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

## macro_connector_flow_status_impls!

**The macro GRACE never mentioned, and the one every connector needs.** All 112 connectors at HEAD
invoke it. The framework's `ConnectorServiceTrait` requires a `ConnectorIntegrationV2` impl for
*every* flow marker in `crates/types-traits/domain_types/src/connector_flow.rs`. You implement six
or eight of them; this macro generates the rest as stubs whose `get_url` returns
`IntegrationError::connector_flow_not_implemented(..)` or `connector_flow_not_supported(..)`.

Real invocation (`connectors/travelhub.rs`):

```rust
macros::macro_connector_flow_status_impls!(
    connector: Travelhub,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept,
        ClientAuthenticationToken,
        CreateConnectorCustomer,
        GetConnectorCustomer,
        DefendDispute,
        MandateRevoke,
        Authenticate,
        IncrementalAuthorization,
        CreateOrder,
        PostAuthenticate,
        PreAuthenticate,
        PaymentMethodToken,
        VoidPC,
        RepeatPayment,
        ServerAuthenticationToken,
        ServerSessionAuthenticationToken,
        SetupMandate,
        SubmitEvidence
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
```

### Argument keys

| Key | Meaning |
|-----|---------|
| `connector` | Connector struct name |
| `generic_type` | Always `T` |
| `[bounds]` | Always `[PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]` |
| `not_implemented` | Flows the **vendor supports** but this integration has not built yet |
| `not_supported` | Flows the **vendor does not offer at all** |

`not_implemented` and `not_supported` are each optional -- the macro has separate entry arms for
both-lists, `not_implemented`-only, and `not_supported`-only. The distinction is semantic and
reviewers check it: `not_implemented` is a TODO, `not_supported` is a permanent property of the
vendor's API.

**How to build your two lists:** take the full marker list from
`crates/types-traits/domain_types/src/connector_flow.rs`, subtract every flow you passed to
`macro_connector_implementation!` / `macro_connector_local_flow_implementation!`, and subtract the
nine payout flows if you invoked `macro_connector_payout_implementation!`. Everything left goes in
one of the two lists. Omitting a flow is `E0277: the trait bound ... is not satisfied`; listing a
flow you also implemented is a conflicting-implementation error.

---

## macro_connector_payout_implementation!

Generates stub impls for the payout flows. Called with no `payout_flows` key, it expands to all
nine: `PayoutCreate`, `PayoutTransfer`, `PayoutGet`, `PayoutVoid`, `PayoutStage`,
`PayoutCreateLink`, `PayoutCreateRecipient`, `PayoutEnrollDisburseAccount`, `PayoutEligibility`.

Real invocation (`connectors/flywire.rs`, `connectors/travelhub.rs` -- identical shape):

```rust
macros::macro_connector_payout_implementation!(
    connector: Flywire,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);
```

| Key | Meaning |
|-----|---------|
| `connector` | Connector struct name |
| `generic_type` | Always `T` |
| `[bounds]` | Always `[PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]` |
| `payout_flows` | *Optional.* An explicit subset, e.g. `payout_flows: [PayoutCreate, PayoutGet]`. Omit it to get all nine |

Pass `payout_flows` only when the connector really implements some payout flows itself -- the
listed flows get stubs, so any flow you implement by hand must be **left out** of the list.

---

## macro_connector_local_flow_implementation!

For a flow that makes **no outbound HTTP call** -- the entire response is built locally in a
connector-owned handler. The macro emits `get_call_connector_action` ->
`CallConnectorAction::HandleResponseWithoutBuildRequest`, `build_request_v2` -> `Ok(None)`, a
`get_url` that is unreachable, and a `handle_response_v2` that forwards to your function.

Real invocation (`connectors/worldpayxml.rs`):

```rust
macros::macro_connector_local_flow_implementation!(
    connector: Worldpayxml,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    handle_response: worldpayxml::handle_pre_authenticate_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);
```

| Key | Meaning |
|-----|---------|
| `connector` | Connector struct name |
| `flow_name` | Flow marker |
| `resource_common_data` / `flow_request` / `flow_response` | Same three types as `macro_connector_implementation!` |
| `handle_response` | Path to a connector-owned function (typically in `transformers.rs`) |
| `generic_type` / `[bounds]` | As everywhere else |

`handle_response` must have exactly this signature:

```rust
fn(
    data: &RouterDataV2<Flow, ResourceCommonData, Request, Response>,
    event_builder: Option<&mut Event>,
    res: Response,
) -> CustomResult<RouterDataV2<Flow, ResourceCommonData, Request, Response>, ConnectorError>
```

The connector file still owns the marker-trait impl itself (e.g.
`impl PaymentPreAuthenticateV2<G> for C<G> {}`); this macro does not emit it. A flow covered here
must **not** also appear in `macro_connector_flow_status_impls!`.

---

## The Non-Generic Trait Impls (not macros, but always needed)

`SourceVerification` and `BodyDecoding` are **non-generic** traits -- they take no flow/data type
parameters. Write exactly **one** impl of each per connector, never one per flow:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for Travelhub<T>
{
}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for Travelhub<T>
{
}
```

Writing `impl<T> SourceVerification<Flow, Data, Req, Resp> for X<T> {}` per flow is
**E0107: trait takes 0 generic arguments but 4 were supplied**. The trait definitions are
`crates/types-traits/interfaces/src/verification.rs` and
`crates/types-traits/interfaces/src/decode.rs`; the exemplar is
`connectors/travelhub.rs`. Every method on both traits has a working default, so the empty impl
body is the correct starting point -- override `get_algorithm` / `get_secrets` / `get_signature` /
`get_message` only when the connector actually signs its webhooks.

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

There are **five** unit types (`crates/common/common_utils/src/types.rs`), each with a matching
`...ForConnector` convertor:

| Type | Convertor | Wire form | Connectors declaring it at HEAD |
|------|-----------|-----------|--------------------------------|
| `StringMajorUnit` | `StringMajorUnitForConnector` | `"10.00"` | 25 |
| `FloatMajorUnit` | `FloatMajorUnitForConnector` | `10.00` | 22 |
| `MinorUnit` | `MinorUnitForConnector` | `1000` | 11 |
| `StringMinorUnit` | `StringMinorUnitForConnector` | `"1000"` | 10 |
| `StringTwoDecimalUnit` | `StringTwoDecimalUnitForConnector` | `"10.00"`, always 2 dp even for zero-decimal currencies | 0 |

**Selection rule: read the vendor's API spec and match its wire format.** Do not default to
`StringMinorUnit` -- of the 67 connectors that declare a converter, only 10 (15%) use it, so
"default to `StringMinorUnit` when unclear" is wrong about 85% of the time. If the spec is
genuinely ambiguous, the sample request/response payloads in the vendor docs settle it: a decimal
point means a *major* unit, quotes mean a *String* variant. `StringTwoDecimalUnit` is the narrow
case where the vendor demands `"10.00"` even for JPY/KWD, where major-unit formatting would
otherwise drop or add decimals.

`amount_converters: []` is legal and common -- use it when the connector's transformers call
`convert_amount(&StringMajorUnitForConnector, ..)` directly rather than going through a
macro-generated wrapper (`flywire.rs`, `travelhub.rs`).

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
    // MissingConnectorTransactionID is a STRUCT variant carrying `context`, not a unit
    // variant. Writing it bare is E0533/E0599.
    _ => return Err(errors::IntegrationError::MissingConnectorTransactionID {
        context: Default::default(),
    }
    .into()),
};
Ok(format!("{}/v1/payments/{}/capture", self.connector_base_url_payments(req), id))
```

Every `IntegrationError` variant carries a `context: IntegrationErrorContext` field (which derives
`Default`), and every `ConnectorError` variant carries `context: ResponseTransformationErrorContext`.
None of them can be written bare. Read the live variant list before choosing one:
`crates/types-traits/domain_types/src/errors.rs`.

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
