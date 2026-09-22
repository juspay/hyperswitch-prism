# UCS Macro-Based Implementation Pattern Reference

## Overview

This document provides comprehensive reference for implementing UCS connectors using the macro-based pattern. The macro pattern significantly reduces boilerplate code and ensures consistency across all flow implementations.

## Core Macros

### 1. `create_all_prerequisites!` - Foundation Setup

This macro sets up the connector foundation, including the connector struct, flow bridges, and amount converters.

**Purpose:**
- Creates the generic connector struct `ConnectorName<T>`
- Sets up bridges for all flows
- Defines amount conversion utilities
- Provides member functions accessible across all flows

**Location:** `crates/integrations/connector-integration/src/connectors/macros.rs`

**Syntax:**
```rust
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    generic_type: {{GenericType}},
    api: [
        (
            flow: {{FlowName}},
            request_body: {{RequestType}},      // Optional - omit for flows without request body
            response_body: {{ResponseType}},
            router_data: {{RouterDataType}},
        ),
        // ... more flows
    ],
    // Pick the unit that matches the vendor's documented wire format. FIVE unit types exist in
    // common_utils/src/types.rs: MinorUnit(:170) `1250`, StringMinorUnit(:305) `"1250"`,
    // FloatMajorUnit(:336) `12.50`, StringMajorUnit(:374) `"12.50"`,
    // StringTwoDecimalUnit(:443) `"12.50"` zero-padded. Do NOT default to StringMinorUnit —
    // on HEAD the split is StringMajorUnit 24 / FloatMajorUnit 22 / MinorUnit 11 / StringMinorUnit 8.
    amount_converters: [
        {{converter_name}}: {{AmountType}},
        // ... more converters if needed
    ],
    member_functions: {
        // Helper methods accessible to all flows
    }
);
```

**Parameters:**
- `connector_name`: The connector struct name (e.g., `Stripe`, `Adyen`)
- `generic_type`: Usually `T` for payment method data generics
- `api`: Array of flow definitions
- `amount_converters`: Array of amount conversion utilities
- `member_functions`: Block containing helper methods

**Flow Definition Parameters:**
- `flow`: Flow type (e.g., `Authorize`, `PSync`, `Capture`, `Refund`, `Void`)
- `request_body`: Request type (can be generic like `StripeRequest<T>` or concrete like `StripeRedirectRequest`)
  - **Omit this parameter** for flows that don't send a request body (e.g., pure GET endpoints)
- `response_body`: Response type
- `router_data`: Full `RouterDataV2` type specification

**Example:**
```rust
macros::create_all_prerequisites!(
    connector_name: Stripe,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: StripePaymentRequest<T>,
            response_body: StripePaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: StripeSyncRequest,
            response_body: StripeSyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: StripeCaptureRequest,
            response_body: StripeCaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: StripeRefundRequest,
            response_body: StripeRefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
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

        pub fn connector_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.stripe.base_url
        }
    }
);
```

### 2. `macro_connector_implementation!` - Flow Implementation

This macro implements the `ConnectorIntegrationV2` trait for a specific flow.

**Purpose:**
- Implements all required methods for a flow
- Handles request body generation
- Handles response parsing
- Auto-implements standard methods like `get_content_type`, `get_error_response_v2`

**Syntax:**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [{{method1}}, {{method2}}, ...],
    connector: {{ConnectorName}},
    curl_request: {{ContentType}}({{RequestType}}),     // Optional - omit for no request body
    curl_response: {{ResponseType}},
    flow_name: {{FlowName}},
    resource_common_data: {{FlowData}},
    flow_request: {{RequestData}},
    flow_response: {{ResponseData}},
    http_method: {{Method}},
    preprocess_response: {{true|false}},                 // Optional - default false
    generic_type: {{GenericType}},
    [{{trait_bounds}}],
    other_functions: {
        // Custom flow-specific methods
    }
);
```

**Parameters:**
- `connector_default_implementations`: Array of default methods to implement (usually `[get_content_type, get_error_response_v2]`)
- `connector`: Connector struct name
- `curl_request`: Request content type and type (e.g., `Json(StripeRequest)`, `FormData(...)`)
  - **Omit this parameter** for flows without request body
- `curl_response`: Response type
- `flow_name`: Flow name (must match flow in `create_all_prerequisites!`)
- `resource_common_data`: Flow-specific common data type
  - `PaymentFlowData` - for payment flows (Authorize, PSync, Capture, Void)
  - `RefundFlowData` - for refund flows (Refund, RSync)
  - `DisputeFlowData` - for dispute flows (Accept, SubmitEvidence, DefendDispute)
- `flow_request`: Request data type from domain_types
- `flow_response`: Response data type from domain_types
- `http_method`: HTTP method (Post, Get, Put, Patch, Delete)
- `preprocess_response`: Optional - set to `true` if connector needs response preprocessing
- `generic_type`: Generic type variable (usually `T`)
- `[trait_bounds]`: Trait bounds for the generic type
- `other_functions`: Block containing flow-specific custom methods

**Content Type Options:**
- `Json(Type)` - For JSON requests
- `FormData(Type)` - For multipart form data
- `FormUrlEncoded(Type)` - For URL-encoded forms
- `RawData(Type)` - For raw data

**Example (With Request Body):**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: Json(StripePaymentRequest),
    curl_response: StripePaymentResponse,
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
            Ok(format!("{}/v1/payment_intents", self.connector_base_url(req)))
        }
    }
);
```

**Example (Without Request Body - Pure GET):**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_response: StripeSyncResponse,
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
            Ok(format!("{}/v1/payment_intents/{}", self.connector_base_url(req), id))
        }
    }
);
```

### 3. `macro_connector_flow_status_impls!` - Stubs for every flow you did NOT implement

Defined at `crates/integrations/connector-integration/src/connectors/macros.rs:1827`.
**All 111 connectors on HEAD invoke it** (111 of the 111 files in `connectors/` excluding `macros.rs`) — a connector that omits it will not compile, because
`ConnectorServiceTrait` requires a `ConnectorIntegrationV2` impl for every flow, implemented or not.

It emits, for each flow you name, both the marker-trait impl and a stub `ConnectorIntegrationV2`
impl that fails with the right error. Two keys, both optional but at least one required:

- `not_implemented: [...]` — the connector's API *could* support this flow, nobody has written it yet.
- `not_supported: [...]` — the connector's API has no such capability at all.

**Syntax** (`macros.rs:1829-1836`, the "both lists" arm):
```rust
macros::macro_connector_flow_status_impls!(
    connector: {{ConnectorName}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [ FlowA, FlowB ],
    not_supported: [ FlowC ],
);
```

**Real invocation** (`connectors/travelhub.rs:455`):
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

Flow identifiers are the marker structs in `domain_types::connector_flow`; the macro has one arm
per flow (`expand_flow_status_impl!`, `macros.rs:1945`), so a typo is a compile error, not a
silent no-op. Do **not** hand-write these stubs — remove any you find and list the flow here.

### 4. `macro_connector_local_flow_implementation!` - Flows with no outbound HTTP call

Defined at `macros.rs:2425`. Use it when a flow is resolved entirely inside UCS — no request is
built and nothing is sent to the connector. The macro sets
`get_call_connector_action` to `CallConnectorAction::HandleResponseWithoutBuildRequest`, makes
`build_request_v2` return `Ok(None)`, and routes everything through the free function you name in
`handle_response`.

**Argument keys** (`macros.rs:2426-2434` — all required, in this order):
```rust
macros::macro_connector_local_flow_implementation!(
    connector: {{ConnectorName}},
    flow_name: {{Flow}},
    resource_common_data: {{FlowData}},
    flow_request: {{RequestData}},
    flow_response: {{ResponseData}},
    handle_response: {{module}}::{{handler_fn}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);
```

**Real invocation** (`connectors/kount.rs:390`):
```rust
macros::macro_connector_local_flow_implementation!(
    connector: Kount,
    flow_name: PreAuthenticate,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsPreAuthenticateData<T>,
    flow_response: PaymentsResponseData,
    handle_response: kount::handle_pre_authenticate_response,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
);
```

A flow covered by this macro must NOT also appear in `macro_connector_flow_status_impls!` — that
would be a conflicting impl.

### 5. `macro_connector_payout_implementation!` - Payout flow stubs

Defined at `macros.rs:1448`. Every connector needs `ConnectorIntegrationV2` impls for the payout
flows whether or not it does payouts. Invoked with no flow list, it expands to all nine payout
flows (`macros.rs:1460-1470`): `PayoutCreate`, `PayoutTransfer`, `PayoutGet`, `PayoutVoid`,
`PayoutStage`, `PayoutCreateLink`, `PayoutCreateRecipient`, `PayoutEnrollDisburseAccount`,
`PayoutEligibility`.

**Real invocation, all payout flows stubbed** (`connectors/travelhub.rs:187`, `connectors/kount.rs:402`):
```rust
macros::macro_connector_payout_implementation!(
    connector: {{ConnectorName}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);
```

**Stub only a subset** (arm 2, `macros.rs:1477`) — name the flows you want stubbed and
hand-implement the rest:
```rust
macros::macro_connector_payout_implementation!(
    connector: {{ConnectorName}},
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    payout_flows: [ PayoutStage, PayoutCreateLink, PayoutEligibility ]
);
```

## Flow-Specific Data Types

### Resource Common Data Types
- **PaymentFlowData** - Used for: Authorize, PSync, Capture, Void, VoidPC, SetupMandate
- **RefundFlowData** - Used for: Refund, RSync
- **DisputeFlowData** - Used for: Accept, SubmitEvidence, DefendDispute

### Request Data Types (from domain_types::connector_types)
- **PaymentsAuthorizeData\<T\>** - For Authorize flow
- **PaymentsSyncData** - For PSync flow
- **PaymentsCaptureData** - For Capture flow
- **PaymentVoidData** - For Void flow
- **PaymentsCancelPostCaptureData** - For VoidPC flow
- **RefundsData** - For Refund flow
- **RefundSyncData** - For RSync flow
- **SetupMandateRequestData\<T\>** - For SetupMandate flow
- **AcceptDisputeData** - For Accept flow
- **SubmitEvidenceData** - For SubmitEvidence flow
- **DisputeDefendData** - For DefendDispute flow

### Response Data Types (from domain_types::connector_types)
- **PaymentsResponseData** - For all payment flows
- **RefundsResponseData** - For all refund flows
- **DisputeResponseData** - For all dispute flows

## Complete Connector Template

```rust
// File: crates/integrations/connector-integration/src/connectors/{{connector_name}}.rs

mod test;
pub mod transformers;

use std::{fmt::Debug, marker::{Send, Sync}, sync::LazyLock};
use common_enums::*;
use common_utils::{errors::CustomResult, events, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::*,
    connector_types::*,
    errors,
    payment_method_data::{DefaultPCIHolder, PaymentMethodData, PaymentMethodDataTypes},
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::*,
    utils,
};
use error_stack::report;
use hyperswitch_masking::{Mask, Maskable};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    connector_types::{self, ConnectorValidation},
};
use serde::Serialize;
use transformers::{self as {{connector_name_lower}}, *};

use super::macros;
use crate::{types::ResponseRouterData, with_error_response_body};

pub(crate) mod headers {
    pub(crate) const CONTENT_TYPE: &str = "Content-Type";
    pub(crate) const AUTHORIZATION: &str = "Authorization";
}

// Trait implementations
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ConnectorServiceTrait<T> for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAuthorizeV2<T> for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSyncV2 for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentCapture for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentVoidV2 for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundV2 for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RefundSyncV2 for {{ConnectorName}}<T>
{}

// Create prerequisites - Foundation setup
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: {{ConnectorName}}PaymentRequest<T>,
            response_body: {{ConnectorName}}PaymentResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        (
            flow: PSync,
            request_body: {{ConnectorName}}SyncRequest,
            response_body: {{ConnectorName}}SyncResponse,
            router_data: RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>,
        ),
        (
            flow: Capture,
            request_body: {{ConnectorName}}CaptureRequest,
            response_body: {{ConnectorName}}CaptureResponse,
            router_data: RouterDataV2<Capture, PaymentFlowData, PaymentsCaptureData, PaymentsResponseData>,
        ),
        (
            flow: Void,
            request_body: {{ConnectorName}}VoidRequest,
            response_body: {{ConnectorName}}VoidResponse,
            router_data: RouterDataV2<Void, PaymentFlowData, PaymentVoidData, PaymentsResponseData>,
        ),
        (
            flow: Refund,
            request_body: {{ConnectorName}}RefundRequest,
            response_body: {{ConnectorName}}RefundResponse,
            router_data: RouterDataV2<Refund, RefundFlowData, RefundsData, RefundsResponseData>,
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

        pub fn connector_base_url<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.{{connector_name_lower}}.base_url
        }
    }
);

// ConnectorCommon implementation
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for {{ConnectorName}}<T>
{
    fn id(&self) -> &'static str {
        "{{connector_name_lower}}"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::Minor
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
        let auth = {{connector_name_lower}}::{{ConnectorName}}AuthType::try_from(auth_type)
            .map_err(|_| errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;
        Ok(vec![(
            headers::AUTHORIZATION.to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.{{connector_name_lower}}.base_url.as_ref()
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: {{connector_name_lower}}::{{ConnectorName}}ErrorResponse = res
            .response
            .parse_struct("ErrorResponse")
            .map_err(|_| errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        // `with_error_response_body!` is a crate macro: `use crate::with_error_response_body;`
        // (definition: crates/integrations/connector-integration/src/utils.rs:61). It expands to
        // `if let Some(body) = event_builder { body.set_connector_response(&response); }` — there is no
        // `set_error_response_body` method on `events::Event`.
        with_error_response_body!(event_builder, response);

        // `ErrorResponse` has 13 fields (domain_types/src/router_data.rs:4228) and a
        // `Default` impl right below it — spell out what you set, then `..Default::default()`.
        // Never `unwrap_or_default()` an error code/message: use the NO_ERROR_* consts
        // (common_utils/src/consts.rs:154-156) so the failure is legible in logs.
        // `attempt_status` is `Option<FlowStatus>`, not `Option<AttemptStatus>`, and it must
        // stay `None` here unless the connector's own payload proves a terminal outcome for
        // THIS flow — hardcoding `Some(FlowStatus::Payment(AttemptStatus::Failure))` on the
        // shared error path is what reports a charged payment as FAILURE.
        // `FlowStatus` is `domain_types::router_data::FlowStatus` (router_data.rs:4186):
        //     use domain_types::router_data::FlowStatus;
        // Variants: Payment(AttemptStatus) | Refund(RefundStatus) | Dispute(DisputeStatus) |
        // Payout(PayoutStatus). Pick the one matching THIS flow.
        Ok(ErrorResponse {
            status_code: res.status_code,
            // `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`: `use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};`
            // (crates/common/common_utils/src/consts.rs:154-156).
            code: response
                .error_code
                .clone()
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .clone()
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.message,
            connector_transaction_id: response.transaction_id,
            ..Default::default()
        })
    }
}

// Flow implementations using macros
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {{ConnectorName}},
    curl_request: Json({{ConnectorName}}PaymentRequest),
    curl_response: {{ConnectorName}}PaymentResponse,
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
            Ok(format!("{}/v1/payments", self.connector_base_url(req)))
        }
    }
);

// Additional flows follow the same pattern...
```

## Best Practices

### 1. **Consistent Naming**
- Request types: `{{ConnectorName}}{{Flow}}Request` (e.g., `StripePaymentRequest`)
- Response types: `{{ConnectorName}}{{Flow}}Response` (e.g., `StripePaymentResponse`)
- Generic requests: Add `<T>` for payment method generics (e.g., `StripePaymentRequest<T>`)

### 2. **Amount Converters**
- **Read the vendor's API spec and match its wire format.** There is no safe default; guessing
  `StringMinorUnit` is wrong for roughly seven out of eight connectors. The distribution across
  the 65 connectors on HEAD that declare a converter is `StringMajorUnit` 24, `FloatMajorUnit` 22,
  `MinorUnit` 11, `StringMinorUnit` 8. Reproduce with:
  `grep -roh 'amount_converter: [A-Za-z]*' crates/integrations/connector-integration/src/connectors/ | sort | uniq -c | sort -rn`
- The five unit types live in `crates/common/common_utils/src/types.rs`:
  | Type | Wire shape | Spec looks like |
  |---|---|---|
  | `MinorUnit` (`types.rs:170`) | JSON number of minor units | `"amount": 1250` |
  | `StringMinorUnit` (`types.rs:305`) | string of minor units | `"amount": "1250"` |
  | `StringMajorUnit` (`types.rs:374`) | string of major units | `"amount": "12.50"` |
  | `FloatMajorUnit` (`types.rs:336`) | JSON float of major units | `"amount": 12.50` |
  | `StringTwoDecimalUnit` (`types.rs:443`) | string, always 2 decimals | `"amount": "12.50"` (zero-padded) |
- Name converter logically (e.g., `amount_converter`, `amount_converter_webhooks`)

### 3. **Member Functions**
- Always include `build_headers` for consistent authentication
- Include flow-specific base URL getters if needed
- Keep helper functions generic with `<F, FCD, Req, Res>` when possible

### 4. **Error Handling**
- Always include `get_error_response_v2` in default implementations
- Parse connector-specific error formats in `build_error_response`

### 5. **Resource Common Data Selection**
```rust
// Payment operations
PaymentFlowData: Authorize, PSync, Capture, Void, VoidPC, SetupMandate

// Refund operations
RefundFlowData: Refund, RSync

// Dispute operations
DisputeFlowData: Accept, SubmitEvidence, DefendDispute
```

## Migration from Manual to Macro Pattern

If you have existing manual implementations, convert them using this mapping:

**Before (Manual):**
```rust
impl<T> ConnectorIntegrationV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
    for Stripe<T>
{
    fn get_headers(&self, req: &RouterDataV2<...>) -> CustomResult<...> { ... }
    fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, ...> { ... }
    fn get_request_body(&self, req: &RouterDataV2<...>) -> CustomResult<...> { ... }
    fn handle_response_v2(&self, data: &RouterDataV2<...>, ...) -> CustomResult<...> { ... }
    // ... more methods
}
```

**After (Macro):**
```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: Stripe,
    curl_request: Json(StripePaymentRequest),
    curl_response: StripePaymentResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<...>) -> CustomResult<...> { ... }
        fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, ...> { ... }
    }
);
```

The macro automatically handles:
- `get_request_body` generation
- `handle_response_v2` implementation
- Request/response transformations via bridge pattern
- Type conversions and error handling

## Common Issues and Solutions

### Issue 1: Generic Type Mismatch
**Problem:** Request type needs to be generic but isn't specified correctly

**Solution:**
```rust
// Wrong
request_body: StripePaymentRequest,

// Correct
request_body: StripePaymentRequest<T>,
```

### Issue 2: Wrong Resource Common Data
**Problem:** Using `PaymentFlowData` for refund flows

**Solution:**
```rust
// Wrong - Refund with PaymentFlowData
flow_name: Refund,
resource_common_data: PaymentFlowData,  // ❌

// Correct
flow_name: Refund,
resource_common_data: RefundFlowData,   // ✅
```

### Issue 3: Missing Flow in Prerequisites
**Problem:** Using flow in `macro_connector_implementation!` but not defined in `create_all_prerequisites!`

**Solution:** Always define flow in both places:
```rust
// 1. Define in create_all_prerequisites!
macros::create_all_prerequisites!(
    api: [
        (flow: Authorize, ...),  // ✅ Defined
    ],
    ...
);

// 2. Then use in macro_connector_implementation!
macros::macro_connector_implementation!(
    flow_name: Authorize,  // ✅ Must match
    ...
);
```

## Macro Expansion Understanding

When you write:
```rust
macros::create_all_prerequisites!(
    connector_name: Stripe,
    ...
);
```

The macro generates:
- `pub struct Stripe<T> { ... }` - The connector struct
- `pub struct StripeRouterData<RD, T> { ... }` - Input data wrapper
- Bridge implementations for request/response handling
- Amount converter wrappers

When you write:
```rust
macros::macro_connector_implementation!(
    flow_name: Authorize,
    ...
);
```

The macro generates:
- Complete `ConnectorIntegrationV2` trait implementation
- `get_request_body` method
- `handle_response_v2` method
- Default method implementations specified in `connector_default_implementations`

This allows you to focus on:
1. Defining what flows exist
2. Defining request/response types
3. Implementing flow-specific logic (headers, URL construction)
4. Writing transformers for request/response conversion
