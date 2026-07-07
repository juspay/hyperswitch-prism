# CreateConnectorCustomer Flow Pattern

## Overview

The `CreateConnectorCustomer` flow registers a customer record with the payment gateway so that subsequent flows (`Authorize`, `SetupMandate`, `RepeatPayment`) can reference it via `connector_customer_id`. Unlike `Authorize` it moves no money; it exchanges PII (email, name, phone, description) for a gateway-side identifier. The flow is an orchestrated *side-flow*: when `ValidationTrait::should_create_connector_customer` returns `true` and no `connector_customer_id` is already present on the request, the composite-service/gRPC layer invokes `CreateConnectorCustomer` BEFORE `Authorize` and splices the returned id into the follow-up request (see `crates/internal/composite-service/src/payments.rs:253`).

### Key Components

- **Flow Marker**: Unit struct `CreateConnectorCustomer` at `crates/types-traits/domain_types/src/connector_flow.rs:44`.
- **Trait bound**: `connector_types::CreateConnectorCustomer` at `crates/types-traits/interfaces/src/connector_types.rs:194-202`.
- **Main connector files**: `crates/integrations/connector-integration/src/connectors/{shift4,stripe,stax,authorizedotnet,finix}.rs` (and their `transformers.rs`).
- **Orchestration glue**: `crates/grpc-server/grpc-server/src/server/payments.rs:264` (`Customer::handle_connector_customer`) and `crates/internal/composite-service/src/payments.rs:235` (`create_connector_customer`).
- **State propagation**: `crates/internal/composite-service/src/utils.rs:44` (`get_connector_customer_id`) stitches the response into `ConnectorState.connector_customer_id` which is then read by the next request.

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Connectors with Full Implementation](#connectors-with-full-implementation)
3. [Flow Composition with Authorize](#flow-composition-with-authorize)
4. [Common Implementation Patterns](#common-implementation-patterns)
5. [Connector-Specific Patterns](#connector-specific-patterns)
6. [Code Examples](#code-examples)
7. [Integration Guidelines](#integration-guidelines)
8. [Best Practices](#best-practices)
9. [Common Errors / Gotchas](#common-errors--gotchas)
10. [Testing Notes](#testing-notes)
11. [Cross-References](#cross-references)

## Architecture Overview

### Flow Hierarchy

```
ConnectorIntegrationV2<CreateConnectorCustomer, PaymentFlowData,
                      ConnectorCustomerData, ConnectorCustomerResponse>
  │
  ├── ValidationTrait::should_create_connector_customer()  -> bool
  │     (default false; returns true for connectors that NEED a customer before Authorize)
  │     see crates/types-traits/interfaces/src/connector_types.rs:125
  │
  ├── macros::create_all_prerequisites!(api: [(flow: CreateConnectorCustomer, ...)])
  │     registers the request/response pair and the RouterDataV2 quadruple
  │
  └── macros::macro_connector_implementation!(flow_name: CreateConnectorCustomer, ...)
        generates ConnectorIntegrationV2 with get_headers / get_url / build_request_body /
        handle_response_v2 / get_error_response_v2
```

### Flow Type

Marker `CreateConnectorCustomer` at `crates/types-traits/domain_types/src/connector_flow.rs:44`:

```rust
// From crates/types-traits/domain_types/src/connector_flow.rs:43-44
#[derive(Debug, Clone)]
pub struct CreateConnectorCustomer;
```

It also appears in the `FlowName` enum at `crates/types-traits/domain_types/src/connector_flow.rs:117`, which is how the orchestrator tags events (`FlowName::CreateConnectorCustomer`).

### Request Type

`ConnectorCustomerData` at `crates/types-traits/domain_types/src/connector_types.rs:1719-1728`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1719
#[derive(Debug, Clone)]
pub struct ConnectorCustomerData {
    pub customer_id: Option<Secret<String>>,
    pub email: Option<Secret<Email>>,
    pub name: Option<Secret<String>>,
    pub description: Option<String>,
    pub phone: Option<Secret<String>>,
    pub preprocessing_id: Option<String>,
    pub split_payments: Option<SplitPaymentsRequest>,
}
```

Every field is optional; connectors choose which to forward. `preprocessing_id` is used by Stripe to attach a tokenized source at creation time (see `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:4690`).

### Response Type

`ConnectorCustomerResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1730-1733`:

```rust
// From crates/types-traits/domain_types/src/connector_types.rs:1730
#[derive(Debug, Clone)]
pub struct ConnectorCustomerResponse {
    pub connector_customer_id: String,
}
```

The single field is the gateway-assigned id; transformers produce it from the connector-native payload (e.g. Stripe `id`, Shift4 `id`, Stax `id`, Authorize.Net `customerProfileId`, Finix `id`).

### Resource Common Data

`PaymentFlowData` at `crates/types-traits/domain_types/src/connector_types.rs:422-464`. The field that matters for this flow is `connector_customer: Option<String>` at `crates/types-traits/domain_types/src/connector_types.rs:425` — it is populated from the `CreateConnectorCustomer` response and read by downstream flows (`Authorize`, `SetupMandate`, `RepeatPayment`). The accessor `PaymentFlowData::get_connector_customer_id` at `crates/types-traits/domain_types/src/connector_types.rs:897` returns an error when it is missing; the setter at `crates/types-traits/domain_types/src/connector_types.rs:939` fills it only when currently `None`.

### Canonical RouterDataV2 signature

```rust
// From crates/types-traits/interfaces/src/connector_types.rs:194-202
pub trait CreateConnectorCustomer:
    ConnectorIntegrationV2<
    connector_flow::CreateConnectorCustomer,
    PaymentFlowData,
    ConnectorCustomerData,
    ConnectorCustomerResponse,
>
{
}
```

The four type parameters MUST appear in this exact order in every transformer and impl.

## Connectors with Full Implementation

`CreateConnectorCustomer` is a full-bodied flow in five connectors at the pinned SHA. Row 1 is Shift4, which landed the flow in PR #882 and is the canonical reference.

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| shift4 | POST | application/json | `{base_url}/customers` | `Shift4CreateCustomerRequest` (dedicated) | PR #882, canonical example. `crates/integrations/connector-integration/src/connectors/shift4.rs:700-728`; transformer at `crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:65-127` |
| authorizedotnet | POST | application/json | `{base_url}` (root — Authorize.Net uses a single JSON-RPC-style endpoint) | `AuthorizedotnetCreateConnectorCustomerRequest<T>` reuses `AuthorizedotnetZeroMandateRequest<T>` for the profile payload | `preprocess_response: true`. `crates/integrations/connector-integration/src/connectors/authorizedotnet.rs:831-860`; duplicate-customer recovery at `crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:3270-3280` |
| finix | POST | application/json | `{base_url}/identities` | `FinixCreateIdentityRequest` (dedicated; Finix calls customers "identities") | Combined with a second side-flow `PaymentMethodToken` → `/payment_instruments`. `crates/integrations/connector-integration/src/connectors/finix.rs:528-555`; transformer at `crates/integrations/connector-integration/src/connectors/finix/transformers.rs:164-204` |
| stax | POST | application/json | `{base_url}/customer` | `StaxCustomerRequest` (dedicated) | Uses `connector_default_implementations: [get_headers, ...]` so only `get_url` is overridden. `crates/integrations/connector-integration/src/connectors/stax.rs:524-546`; transformer at `crates/integrations/connector-integration/src/connectors/stax/transformers.rs:810-892` |
| stripe | POST | application/x-www-form-urlencoded | `{base_url}v1/customers` | `CreateConnectorCustomerRequest` (dedicated; note the type literally uses the flow's name) | Form-encoded, not JSON. Custom `get_headers` injects `Stripe-Account` for direct-charge split payments. `crates/integrations/connector-integration/src/connectors/stripe.rs:663-719`; transformer at `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:361-377,4656-4714` |

All five connectors additionally implement the marker trait `connector_types::CreateConnectorCustomer` on their struct:

- shift4: `crates/integrations/connector-integration/src/connectors/shift4.rs:537-540`
- authorizedotnet: `crates/integrations/connector-integration/src/connectors/authorizedotnet.rs:115`
- finix: `crates/integrations/connector-integration/src/connectors/finix.rs:161-163`
- stax: `crates/integrations/connector-integration/src/connectors/stax.rs:590-593`
- stripe: `crates/integrations/connector-integration/src/connectors/stripe.rs:100`

### Stub Implementations

Every connector in `crates/integrations/connector-integration/src/connectors/` that implements `ConnectorServiceTrait<T>` compiles an empty `ConnectorIntegrationV2<CreateConnectorCustomer, ...>` impl to satisfy the trait bound, even when no real flow exists. Those stubs must NOT appear in the full-implementation table. They are recognisable because they return `ValidationTrait::should_create_connector_customer() -> false` (the default from `crates/types-traits/interfaces/src/connector_types.rs:125`).

## Flow Composition with Authorize

The whole point of this flow is that it *precedes* `Authorize`. Two distinct orchestrators exercise it.

### 1. Composite authorize (single gRPC call triggers both)

`crates/internal/composite-service/src/payments.rs:235-277` defines `create_connector_customer`, which is invoked from `process_composite_authorize` at `crates/internal/composite-service/src/payments.rs:308-337`:

```rust
// From crates/internal/composite-service/src/payments.rs:253-276
let should_create_connector_customer =
    connector_data.connector.should_create_connector_customer()
        && connector_customer_id.is_none();

let create_customer_response = match should_create_connector_customer {
    true => {
        let create_customer_payload =
            grpc_api_types::payments::CustomerServiceCreateRequest::foreign_from(payload);
        let mut create_customer_request = tonic::Request::new(create_customer_payload);
        *create_customer_request.metadata_mut() = metadata.clone();
        *create_customer_request.extensions_mut() = extensions.clone();

        let create_customer_response = self
            .customer_service
            .create(create_customer_request)
            .await?
            .into_inner();

        Some(create_customer_response)
    }
    false => None,
};
```

The resulting id is then grafted onto the `Authorize` request inside the transformer at `crates/internal/composite-service/src/transformers.rs:76-93`:

```rust
// From crates/internal/composite-service/src/transformers.rs:76-93
let connector_customer_id_from_req = item
    .state
    .as_ref()
    .and_then(|state| state.connector_customer_id.clone());

let connector_customer_id =
    get_connector_customer_id(connector_customer_id_from_req, create_customer_response);

// ...

let resolved_state = Some(ConnectorState {
    access_token,
    connector_customer_id,
});
```

`get_connector_customer_id` itself (at `crates/internal/composite-service/src/utils.rs:44-50`) prefers the request-supplied id and falls back to the `CreateConnectorCustomer` response. This means idempotency: if the caller already has a stored `connector_customer_id`, the side-flow is skipped.

### 2. Standalone gRPC `CustomerService::create`

`crates/grpc-server/grpc-server/src/server/payments.rs:504-661` exposes `CustomerService::create`. It is a single-flow RPC — callers who manage their own customer lifecycle invoke it directly and pass the returned id into subsequent `Authorize` requests. Internally it constructs the same `RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>` described above (`crates/grpc-server/grpc-server/src/server/payments.rs:598-609`).

### Consumption by Authorize / RepeatPayment

Once `PaymentFlowData.connector_customer` is populated, Authorize-side transformers read it:

- Shift4 RepeatPayment consumes it at `crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:909-912`:

```rust
// From crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:908-913
(
    Shift4RepeatPaymentCard::Token(token),
    item.resource_common_data.connector_customer.clone(),
)
```

- Authorize.Net's Authorize transformer consumes it at `crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:784-811` via the `customer_profile_id` wrapping on the paymentProfile.

The contract is: `CreateConnectorCustomer` writes `connector_customer_id` → orchestrator stores it on `ConnectorState` → next request re-hydrates it into `PaymentFlowData.connector_customer` → Authorize transformer reads `item.resource_common_data.connector_customer`.

## Common Implementation Patterns

### Pattern A — Macro-based implementation (all five full connectors)

Every full implementation uses the macro pair:

1. Register the flow in `create_all_prerequisites!`:

```rust
// From crates/integrations/connector-integration/src/connectors/shift4.rs:243-248
(
    flow: CreateConnectorCustomer,
    request_body: Shift4CreateCustomerRequest,
    response_body: Shift4CreateCustomerResponse,
    router_data: RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
),
```

2. Generate the `ConnectorIntegrationV2` impl with `macro_connector_implementation!` (see §6 for the full block).

3. Implement the marker trait on the connector struct so it satisfies `ConnectorServiceTrait<T>`:

```rust
// From crates/integrations/connector-integration/src/connectors/shift4.rs:537-540
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::CreateConnectorCustomer for Shift4<T>
{
}
```

4. Override `ValidationTrait::should_create_connector_customer` to return `true` (the default is `false`):

```rust
// From crates/integrations/connector-integration/src/connectors/stax.rs:221-223
fn should_create_connector_customer(&self) -> bool {
    true
}
```

### Pattern B — Request-body serialization shape

Two serialization shapes appear:

- **JSON** (shift4, authorizedotnet, finix, stax) — `curl_request: Json(...)`.
- **application/x-www-form-urlencoded** (stripe) — `curl_request: FormUrlEncoded(CreateConnectorCustomerRequest)` at `crates/integrations/connector-integration/src/connectors/stripe.rs:666`. Stripe's API historically accepts only form encoding on `/v1/customers`.

Choose based on the connector's OpenAPI spec; never guess.

### Pattern C — URL shape

- Collection-create style (`POST {base}/customers`): shift4 (`crates/integrations/connector-integration/src/connectors/shift4.rs:725`), stripe (`crates/integrations/connector-integration/src/connectors/stripe.rs:716`), stax (`crates/integrations/connector-integration/src/connectors/stax.rs:543`).
- Domain-rename style (`POST {base}/identities`): finix (`crates/integrations/connector-integration/src/connectors/finix.rs:552`).
- Single-endpoint RPC style (`POST {base}` with action implied by body): authorizedotnet (`crates/integrations/connector-integration/src/connectors/authorizedotnet.rs:857`).

### Pattern D — Response → `ConnectorCustomerResponse` mapping

The transformer `TryFrom<ResponseRouterData<..., Self>> for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>` must produce exactly one field:

```rust
// From crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:112-127
impl<F, T> TryFrom<ResponseRouterData<Shift4CreateCustomerResponse, Self>>
    for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>
{
    type Error = error_stack::Report<ConnectorResponseTransformationError>;

    fn try_from(
        item: ResponseRouterData<Shift4CreateCustomerResponse, Self>,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            response: Ok(ConnectorCustomerResponse {
                connector_customer_id: item.response.id,
            }),
            ..item.router_data
        })
    }
}
```

The generic `F, T` form lets the same `TryFrom` cover both the canonical `RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>` and the re-invocation shape emitted by the macro framework.

## Connector-Specific Patterns

### shift4 (canonical — PR #882)

- Minimal request (`email`, `description`); see `crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:65-77`.
- Authorize trait stack (`PaymentTokenV2`, `CreateConnectorCustomer`, `RepeatPaymentV2`, `ConnectorServiceTrait`) is implemented in one block at `crates/integrations/connector-integration/src/connectors/shift4.rs:532-558`; placing them together is the convention the reviewer expects.
- Downstream consumption in `Shift4RepeatPaymentRequest` at `crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:908-934` uses `item.resource_common_data.connector_customer` when the MIT path selects a stored-card token.

### stripe

- Form encoding (not JSON) — `curl_request: FormUrlEncoded(CreateConnectorCustomerRequest)` at `crates/integrations/connector-integration/src/connectors/stripe.rs:666`.
- Extra header `Stripe-Account` is injected only when `split_payments` is `StripeSplitPayment` with `charge_type = Stripe(Direct)` — see `crates/integrations/connector-integration/src/connectors/stripe.rs:686-706`. This deviates from the default `get_headers` because the split-payment flow targets a connected account.
- `preprocessing_id` from `ConnectorCustomerData` is forwarded as `source` so that a token minted during `PaymentMethodToken` can be attached at customer creation (`crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:4690-4695`).

### authorizedotnet

- The request body is the full `createCustomerProfileRequest` envelope (`AuthorizedotnetCreateConnectorCustomerRequest<T>` wraps `AuthorizedotnetZeroMandateRequest<T>`, `crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:1610-1627`).
- The macro is configured with `preprocess_response: true` at `crates/integrations/connector-integration/src/connectors/authorizedotnet.rs:842`, because Authorize.Net returns its body with a BOM that must be stripped before JSON parsing.
- Duplicate-customer recovery: when the API returns error code `E00039`, the response transformer extracts the existing profile id from the error text so the caller still receives a usable `connector_customer_id` (`crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:3270-3290`).

### finix

- Finix calls the resource an "identity", not a "customer"; the request struct is `FinixCreateIdentityRequest` with nested `FinixIdentityEntity` (`crates/integrations/connector-integration/src/connectors/finix/transformers.rs:164-204`).
- Both `CreateConnectorCustomer` AND `PaymentMethodToken` side-flows are enabled (`crates/integrations/connector-integration/src/connectors/finix.rs:126-146`): the orchestrator calls them in order Customer → Token → Authorize.
- `identity_type: PERSONAL` is hard-coded in the enum (`crates/integrations/connector-integration/src/connectors/finix/transformers.rs:191-194`); business identities are not supported by this connector at the pinned SHA.

### stax

- Stax enforces required-field validation inside the transformer (`crates/integrations/connector-integration/src/connectors/stax/transformers.rs:844-854`), returning `IntegrationError::MissingRequiredField { field_name: "email" }` or `"name"` when absent. This is the exception — other connectors pass `None` through.
- Stax reuses `get_headers` from the default set (`connector_default_implementations: [get_headers, ...]` at `crates/integrations/connector-integration/src/connectors/stax.rs:526`) because no flow-specific header is required.
- The response id is wrapped in `Secret<String>` (`crates/integrations/connector-integration/src/connectors/stax/transformers.rs:870-872`) and unwrapped with `.expose()` when mapping to `ConnectorCustomerResponse` (`crates/integrations/connector-integration/src/connectors/stax/transformers.rs:887`).

## Code Examples

### Example 1 — Shift4 main connector block (PRIMARY REFERENCE)

```rust
// From crates/integrations/connector-integration/src/connectors/shift4.rs:699-728
// Create Connector Customer
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Shift4,
    curl_request: Json(Shift4CreateCustomerRequest),
    curl_response: Shift4CreateCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/customers"))
        }
    }
);
```

### Example 2 — Shift4 request/response transformers

```rust
// From crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:65-110
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Shift4CreateCustomerRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<pii::Email>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Shift4CreateCustomerResponse {
    pub id: String,
}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        Shift4RouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    > for Shift4CreateCustomerRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: Shift4RouterData<
            RouterDataV2<
                CreateConnectorCustomer,
                PaymentFlowData,
                ConnectorCustomerData,
                ConnectorCustomerResponse,
            >,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            email: item.router_data.request.email.clone().expose_option(),
            description: item.router_data.request.description.clone(),
        })
    }
}
```

### Example 3 — Stripe form-encoded request + split-payment header

```rust
// From crates/integrations/connector-integration/src/connectors/stripe.rs:663-719
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: FormUrlEncoded(CreateConnectorCustomerRequest),
    curl_response: CreateConnectorCustomerResponse,
    flow_name: CreateConnectorCustomer,
    resource_common_data: PaymentFlowData,
    flow_request: ConnectorCustomerData,
    flow_response: ConnectorCustomerResponse,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + std::marker::Sync + std::marker::Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                headers::CONTENT_TYPE.to_string(),
                self.common_get_content_type().to_string().into(),
            )];
            let transfer_account_id = req
                .request
                .split_payments
                .as_ref()
                .map(|split_payments| {
                    let domain_types::connector_types::SplitPaymentsRequest::StripeSplitPayment(stripe_split_payment) =
                        split_payments;
                    stripe_split_payment
                })
                .filter(|stripe_split_payment| {
                    matches!(stripe_split_payment.charge_type, common_enums::PaymentChargeType::Stripe(common_enums::StripeChargeType::Direct))
                })
                .map(|stripe_split_payment| stripe_split_payment.transfer_account_id.clone());

            if let Some(transfer_account_id) = transfer_account_id {
                let mut customer_account_header = vec![(
                    headers::STRIPE_COMPATIBLE_CONNECT_ACCOUNT.to_string(),
                    transfer_account_id.clone().into_masked(),
                )];
                header.append(&mut customer_account_header);
            };

            let mut api_key = self.get_auth_header(&req.connector_config)?;
            header.append(&mut api_key);
            Ok(header)
        }
        fn get_url(
            &self,
            req: &RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>,
        ) -> CustomResult<String, IntegrationError> {
            Ok(format!("{}{}", self.connector_base_url_payments(req), "v1/customers"))
        }
    }
);
```

### Example 4 — Orchestrator gating (side-flow composition)

```rust
// From crates/internal/composite-service/src/payments.rs:243-276
let connector_customer_id = payload
    .state
    .as_ref()
    .and_then(|state| state.connector_customer_id.as_ref())
    .or_else(|| {
        payload
            .customer
            .as_ref()
            .and_then(|c| c.connector_customer_id.as_ref())
    });
let should_create_connector_customer =
    connector_data.connector.should_create_connector_customer()
        && connector_customer_id.is_none();

let create_customer_response = match should_create_connector_customer {
    true => {
        let create_customer_payload =
            grpc_api_types::payments::CustomerServiceCreateRequest::foreign_from(payload);
        let mut create_customer_request = tonic::Request::new(create_customer_payload);
        *create_customer_request.metadata_mut() = metadata.clone();
        *create_customer_request.extensions_mut() = extensions.clone();

        let create_customer_response = self
            .customer_service
            .create(create_customer_request)
            .await?
            .into_inner();

        Some(create_customer_response)
    }
    false => None,
};
```

## Integration Guidelines

1. Confirm the connector API actually has a customer/profile/identity endpoint. If it does not, do NOT implement this flow; leave the empty `ConnectorIntegrationV2<CreateConnectorCustomer, ...>` stub and keep `should_create_connector_customer` at its default `false`.
2. Add `CreateConnectorCustomer` to the flow-marker import list (see `crates/integrations/connector-integration/src/connectors/shift4.rs:11`).
3. Define a dedicated request struct and a response struct in `transformers.rs`. Keep the request minimal — only include fields your API actually consumes; do not emit `None` defaults.
4. Write `TryFrom<{Connector}RouterData<RouterDataV2<CreateConnectorCustomer, PaymentFlowData, ConnectorCustomerData, ConnectorCustomerResponse>, T>> for {Connector}CreateCustomerRequest` that unwraps `ConnectorCustomerData`'s `Secret`-wrapped fields with `.expose_option()` / `.peek()` exactly as required by the connector.
5. Write `TryFrom<ResponseRouterData<{Connector}CreateCustomerResponse, Self>> for RouterDataV2<F, PaymentFlowData, T, ConnectorCustomerResponse>` that writes the id to `ConnectorCustomerResponse.connector_customer_id` and spreads the rest (`..item.router_data`).
6. Register the flow in `macros::create_all_prerequisites!` with the canonical four-argument `RouterDataV2` and with the chosen request/response structs (see `crates/integrations/connector-integration/src/connectors/shift4.rs:243-248`).
7. Emit the `ConnectorIntegrationV2` impl via `macros::macro_connector_implementation!` with `flow_name: CreateConnectorCustomer`, `flow_request: ConnectorCustomerData`, `flow_response: ConnectorCustomerResponse`, `resource_common_data: PaymentFlowData`, `http_method: Post`, and a `get_url` that points at the creation endpoint.
8. Implement the marker trait `connector_types::CreateConnectorCustomer` on the connector struct.
9. Override `ValidationTrait::should_create_connector_customer` to return `true`.
10. Verify composition: run the `create_customer_suite` global test suite at `crates/internal/ucs-connector-tests/src/global_suites/create_customer_suite/scenario.json`, then run an Authorize scenario and confirm `connector_customer_id` is reused.

## Best Practices

- Always return `connector_customer_id` as a plain `String`; the type is unmasked on purpose because the id is safe to log (see `ConnectorCustomerResponse` at `crates/types-traits/domain_types/src/connector_types.rs:1730-1733`). Stax unwraps `Secret<String>` deliberately before storing (`crates/integrations/connector-integration/src/connectors/stax/transformers.rs:884-887`).
- Idempotency: use the connector's "lookup by merchant_customer_id" or duplicate-handling path when available. Authorize.Net's E00039 recovery (`crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:3270-3290`) is the canonical example — never let a duplicate error abort the Authorize chain.
- Do not hardcode `customer_id` from the merchant side as the `connector_customer_id`; the gateway assigns its own id. Forward the merchant id as `description` or `merchant_customer_id` only when the API documents such a field (see `Profile.merchant_customer_id` at `crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:1648`).
- Keep side-flows side-effect-free on errors. If the customer-create call fails, the orchestrator at `crates/grpc-server/grpc-server/src/server/payments.rs:378-390` surfaces the error as `CONNECTOR_CUSTOMER_CREATION_ERROR`; do not mutate `PaymentFlowData.connector_customer` in that branch.
- Pair with `pattern_payment_method_token.md` when both flows are required: Finix shows the correct order (Customer → Token → Authorize) at `crates/integrations/connector-integration/src/connectors/finix.rs:126-146`.
- Pair with `pattern_setup_mandate.md` for MIT setup. `handle_connector_customer_for_setup_mandate` at `crates/grpc-server/grpc-server/src/server/payments.rs:394-501` is the dedicated entry point that runs CreateConnectorCustomer inside a SetupMandate call.

## Common Errors / Gotchas

1. **Problem**: `should_create_connector_customer` left at default `false`, so the orchestrator never calls the flow and `Authorize` fails because `PaymentFlowData.connector_customer` is `None`.
   **Solution**: Override the method to `true` on the `ValidationTrait` impl (see `crates/integrations/connector-integration/src/connectors/stax.rs:221-223`). The default lives at `crates/types-traits/interfaces/src/connector_types.rs:125`.
2. **Problem**: Missing marker-trait impl `connector_types::CreateConnectorCustomer for {Connector}<T>`. This compiles but the connector fails the `ConnectorServiceTrait<T>` bound because its `create_customer_integration` method returns a bogus stub.
   **Solution**: Add the `impl<T: ...> connector_types::CreateConnectorCustomer for {Connector}<T> {}` block next to the other service traits (pattern: `crates/integrations/connector-integration/src/connectors/shift4.rs:537-540`).
3. **Problem**: Response transformer writes the id back into `customer_id` on the request side instead of `connector_customer_id` on the response side. Downstream `Authorize` then sees `PaymentFlowData.connector_customer = None`.
   **Solution**: Always assign `ConnectorCustomerResponse { connector_customer_id: ... }` inside `response: Ok(...)` on the `RouterDataV2`, as in `crates/integrations/connector-integration/src/connectors/shift4/transformers.rs:120-125`.
4. **Problem**: Using JSON when the gateway only accepts form-encoding (Stripe) — the request 400s with "Invalid content type".
   **Solution**: Use `curl_request: FormUrlEncoded(...)` (`crates/integrations/connector-integration/src/connectors/stripe.rs:666`), not `Json(...)`. Inspect the connector's OpenAPI spec first.
5. **Problem**: Duplicate-customer errors abort Authorize. Authorize.Net returns E00039 when a merchant_customer_id collides.
   **Solution**: Parse the existing id out of the error message and return it as `ConnectorCustomerResponse` anyway; the flow has succeeded semantically. Reference: `crates/integrations/connector-integration/src/connectors/authorizedotnet/transformers.rs:3270-3290`.
6. **Problem**: Forwarding `preprocessing_id` verbatim to connectors that do not expect it. The field is a Stripe-only affordance (source-token attachment).
   **Solution**: Only read `item.router_data.request.preprocessing_id` when the connector explicitly documents the field (Stripe: `crates/integrations/connector-integration/src/connectors/stripe/transformers.rs:4690-4695`). Drop it for the others.
7. **Problem**: Returning the id inside a `Secret<String>` breaks logging and `ConnectorState` serialization.
   **Solution**: `.expose()` or unwrap before assignment (`crates/integrations/connector-integration/src/connectors/stax/transformers.rs:887`). The field type is `String`, not `Secret<String>`, by design.

## Testing Notes

- **Unit tests**: Each transformer `TryFrom` should have a unit test that builds a synthetic `RouterDataV2<CreateConnectorCustomer, ...>` and asserts the connector-native serialized body matches the documented fixture. Follow the convention used in existing connector test modules under `crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs` (tests live inline with `#[cfg(test)]`).
- **Integration-test shape**: The global suite `create_customer_suite` (`crates/internal/ucs-connector-tests/src/global_suites/create_customer_suite/scenario.json`) exercises the standalone gRPC path. Add the connector to its allow-list after implementation.
- **Composite test shape**: `crates/grpc-server/grpc-server/tests/stripe_payment_flows_test.rs` and `crates/grpc-server/grpc-server/tests/authorizedotnet_payment_flows_test.rs` run the customer → authorize chain end-to-end; mirror one of these for a new full implementation.

| Scenario | Expected outcome |
|----------|------------------|
| Standalone CreateConnectorCustomer with valid email/name | 2xx, `connector_customer_id` populated |
| Standalone CreateConnectorCustomer with missing required field | `IntegrationError::MissingRequiredField` (connector-specific; Stax is the reference) |
| CompositeAuthorize, no stored connector_customer_id, `should_create_connector_customer = true` | Customer created first, id threaded into Authorize, Authorize succeeds |
| CompositeAuthorize, request already carries `state.connector_customer_id` | Customer flow skipped; Authorize uses the supplied id |
| Duplicate customer (Authorize.Net E00039) | Existing id extracted from error text; Authorize proceeds |

## Cross-References

- Parent index: [README.md](./README.md)
- Authoring spec: [PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Flow that consumes the id: [pattern_authorize.md](./pattern_authorize.md)
- Sibling side-flow that pairs with this one: [pattern_payment_method_token.md](./pattern_payment_method_token.md)
- Sibling flow that also pairs with customers: [pattern_setup_mandate.md](./pattern_setup_mandate.md)
- Shared macro reference: [macro_patterns_reference.md](./macro_patterns_reference.md)
- Connector types index: [../types/types.md](../types/types.md)
