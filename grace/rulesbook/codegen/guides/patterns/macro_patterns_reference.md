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
    amount_converters: [
        {{converter_name}}: {{AmountType}},     // e.g., amount_converter: StringMinorUnit
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
            let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
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
- `preprocess_request`: Optional - set to `true` if the connector needs to transform the raw JSON body bytes before sending (e.g. wrap/encrypt/sign it). Implement `preprocess_request_bytes` in `create_all_prerequisites!` `member_functions`. See [Request/Response Preprocessing](#advanced-requestresponse-preprocessing) below.
- `preprocess_response`: Optional - set to `true` if the connector needs response preprocessing (e.g., XML → JSON, or decrypting a JWE envelope). Implement `preprocess_response_bytes` in `create_all_prerequisites!` `member_functions`.
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

## Flow-Specific Data Types

### Resource Common Data Types
- **PaymentFlowData** - Used for: Authorize, PSync, Capture, Void, VoidPC, SetupMandate, Recharge, CreatePaymentMethod, GetPaymentMethod, PaymentMethodEligibility
- **RefundFlowData** - Used for: Refund, RSync
- **DisputeFlowData** - Used for: Accept, SubmitEvidence, DefendDispute
- **MerchantAuthenticationFlowData** - Used for: ServerAuthenticationToken, ServerSessionAuthenticationToken, ClientAuthenticationToken (OAuth/login bootstrap calls)
- **FrmFlowData** - Used for: PreRiskCheck, PostRiskCheck, FrmPaymentOutcome, FrmRefundProcessed, FrmChargebackReceived (fraud/FraudAndRiskManagementService flows)

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
- **ServerAuthenticationTokenRequestData** - For ServerAuthenticationToken (OAuth/login bootstrap)
- **RechargeRequestData** - For Recharge (card/wallet top-up, e.g. Qwikcilver)
- **CreatePaymentMethodData** - For CreatePaymentMethod (provision a wallet/payment-method record)
- **GetPaymentMethodData** - For GetPaymentMethod (fetch a wallet/payment-method record)
- **PaymentMethodEligibilityData** - For PaymentMethodEligibility (check a payment method's eligibility)

### Response Data Types (from domain_types::connector_types)
- **PaymentsResponseData** - For all payment flows
- **RefundsResponseData** - For all refund flows
- **DisputeResponseData** - For all dispute flows
- **ServerAuthenticationTokenResponseData** - For ServerAuthenticationToken
- **RechargeResponseData** - For Recharge
- **CreatePaymentMethodResponseData** - For CreatePaymentMethod
- **GetPaymentMethodResponseData** - For GetPaymentMethod
- **PaymentMethodEligibilityResponse** - For PaymentMethodEligibility

## Advanced: Request/Response Preprocessing (e.g. JOSE/JWE wrapping, XML, custom decoding)

A few connectors do not send plain JSON — they require the raw request bytes to be wrapped (encrypted/signed)
before they hit the wire, and/or the raw response bytes to be unwrapped (decrypted/verified) before the
JSON response type is deserialized. Two patterns exist for this:

### `preprocess_response: true` (existing connectors: `worldpayxml`, `twoc_twop_paco`, and others)
Set this flag when the connector's response is not directly deserializable as JSON. You must provide a
`preprocess_response_bytes` member function in `create_all_prerequisites!`:

```rust
pub fn preprocess_response_bytes<F, FCD, Req, Res>(
    &self,
    _req: &RouterDataV2<F, FCD, Req, Res>,
    bytes: bytes::Bytes,
    status_code: u16,
) -> CustomResult<bytes::Bytes, errors::ConnectorError> {
    // convert/decrypt; return the JSON bytes that match `curl_response`'s schema
    Ok(bytes)
}
```

### `preprocess_request: true` (introduced by `twoc_twop_paco`)
Set this flag — **together with** `preprocess_response: true` when the connector also wraps responses —
when the connector's request must be transformed after serialization but before HTTP dispatch. You must
provide a `preprocess_request_bytes` member function. `curl_request` still describes the **logical**
(JSON) request; the macro emits code that passes the serialized JSON bytes through your
`preprocess_request_bytes` before sending.

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_error_response_v2],
    connector: TwocTwopPaco,
    curl_request: Json(TwocTwopPacoAuthorizeRequest),   // logical JSON body
    curl_response: TwocTwopPacoAuthorizeResponse,
    flow_name: Authorize,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsAuthorizeData<T>,
    flow_response: PaymentsResponseData,
    http_method: Post,
    preprocess_request: true,
    preprocess_response: true,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_content_type(&self) -> &'static str {
            "application/jose"            // wire format (not the logical JSON)
        }
        // ...
    }
);
```

And in `create_all_prerequisites!`:
```rust
pub fn preprocess_request_bytes<F, FCD, Req, Res>(
    &self,
    req: &RouterDataV2<F, FCD, Req, Res>,
    bytes: Vec<u8>,
) -> CustomResult<Vec<u8>, errors::IntegrationError> {
    // wrap `bytes` (already JSON). E.g. JOSE sign+encrypt:
    let inner: serde_json::Value = serde_json::from_slice(&bytes)?;
    let claims = PacoJoseClaims::new(auth.access_token.peek(), inner);
    let jwe = common_utils::crypto::jose::sign_then_encrypt(&claims, &auth.jose_cfg)?;
    Ok(jwe.into_bytes())
}
```

**When to use:**
- JOSE/JWS/JWE wrapping (request signing/encryption), e.g. 2C2P PACO → `application/jose`
- XML-in/proto-out or other transport encodings
- Any time the connector's HTTP payload is not the direct JSON of your request type

## Advanced: Local-only flow (no outbound connector call)

Some flows need to return a response **without ever calling the connector's API** — for example,
Kount's `PreAuthenticate` builds a Device-Data-Collection `<script>` locally. Implement
`ConnectorIntegrationV2` directly (no `macro_connector_implementation!`) and return
`CallConnectorAction::HandleResponseWithoutBuildRequest`:

```rust
impl<T: ...> ConnectorIntegrationV2<PreAuthenticate, PaymentFlowData, PaymentsPreAuthenticateData<T>, PaymentsResponseData>
    for Kount<T>
{
    fn get_call_connector_action(&self) -> common_enums::CallConnectorAction {
        common_enums::CallConnectorAction::HandleResponseWithoutBuildRequest
    }

    fn build_request_v2(&self, _req: &RouterDataV2<...>) -> CustomResult<Option<common_utils::request::Request>, IntegrationError> {
        Ok(None)    // nothing is sent to the connector
    }

    fn handle_response_v2(&self, data: &RouterDataV2<...>, _event_builder, _res: Response)
        -> CustomResult<RouterDataV2<...>, ConnectorError>
    {
        // Build the response locally.
        router_data.response = Ok(PaymentsResponseData::PreAuthenticateResponse {
            redirection_data: Some(Box::new(RedirectForm::Script { script_data: script })),
            // ...
            status_code: 200,
        });
        Ok(router_data)
    }
}
```

The `RedirectForm::Script { script_data }` variant is how a connector returns a raw `<script>` blob
(e.g. a DDC collector) that the embedding page executes — distinct from an HTML form redirect.

## Advanced: Pre-auth / composite flows

A connector may need one of these **before** `Authorize`:
- `ServerAuthenticationToken` — OAuth client-credentials or session-token bootstrap. When the rest of
  the API is bearered with the returned token, set `ValidationTrait::should_do_access_token -> true`
  and mark the flow's `resource_common_data` as `MerchantAuthenticationFlowData`. GRPC clients can
  either call the composite HTTP endpoint (which bootstraps the token automatically) or pass the token
  back via `state.access_token`. Example: Qwikcilver `/authorize`.
- `PreAuthenticate` — builds local device-data-collection content (see Kount). Mark the flow in
  `ValidationTrait::next_authentication_step` to return `AuthenticationStep::PreAuthenticate` so the
  composite loop runs it before Authorize.

## Advanced: FRM / Risk-service flows

For fraud-check providers (e.g. Kount), implement:
- `PreRiskCheck` — submit the order for evaluation (`POST`), using `FrmFlowData`
- `PostRiskCheck` — signal "payment authorized" (often unused → `frm_flow_not_implemented!`)
- `FrmPaymentOutcome` / `FrmRefundProcessed` — notify the provider of the final outcome (`PATCH`)
- `FrmChargebackReceived` — provider pushed a dispute (often unused → `frm_flow_not_implemented!`)

Use `macros::frm_flow_not_implemented!` for the ones you don't support, and
`impl connector_types::FrmServiceTrait` for the connector.

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
    router_data::{ConnectorAuthType, ErrorResponse},
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
            let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
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
        auth_type: &ConnectorAuthType,
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
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: {{connector_name_lower}}::{{ConnectorName}}ErrorResponse = res
            .response
            .parse_struct("ErrorResponse")
            .map_err(|_| errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            code: response.error_code.clone(),
            message: response.message.clone(),
            reason: Some(response.message),
            attempt_status: None,
            connector_transaction_id: response.transaction_id,
            network_decline_code: None,
            network_advice_code: None,
            network_error_message: None,
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
- Use `StringMinorUnit` for most connectors
- Use `FloatMajorUnit` if connector requires decimal amounts
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
