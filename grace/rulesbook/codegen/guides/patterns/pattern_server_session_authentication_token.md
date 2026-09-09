# ServerSessionAuthenticationToken Flow Pattern for Connector Implementation

> **Auth mechanism: C — MERCHANT / CREDENTIAL AUTHENTICATION. This is NOT 3DS.**
>
> UCS has three separate authentication mechanisms. Conflating them is the single largest
> codegen risk in this corpus:
>
> | # | Mechanism | Flow markers | `resource_common_data` | gRPC service |
> |---|---|---|---|---|
> | A | Standalone 3DS trio | `PreAuthenticate` / `Authenticate` / `PostAuthenticate` | `PaymentFlowData` | `PaymentMethodAuthenticationService` |
> | B | In-payment 3DS | none — folded into `Authorize` | `PaymentFlowData` | `PaymentService.Authorize` |
> | **C** | **Merchant / credential auth — THIS FILE** | `ServerAuthenticationToken` / `ServerSessionAuthenticationToken` / `ClientAuthenticationToken` | **`MerchantAuthenticationFlowData`** | `MerchantAuthenticationService` |
>
> Every mechanism-C flow binds **`MerchantAuthenticationFlowData`**, never `PaymentFlowData`.
> `MerchantAuthenticationFlowData` lives in `crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`
> and its own doc-comment says why: *"This type deliberately omits payment-specific fields
> (`payment_id`, `attempt_id`, `status`, `payment_method`, `address`, `amount`, etc.) because
> merchant-authentication flows have no payment identity."*
>
> Verify before copying anything below:
> ```bash
> rg -n "pub trait (ServerAuthentication|ServerSessionAuthentication|ClientAuthentication):" -A 9 \
>    crates/types-traits/interfaces/src/connector_types.rs
> ```
>
> External 3DS providers (Netcetera, 3dsecure.io, GPayments, Cardinal, CTP) run entirely inside the
> Hyperswitch router and never reach UCS. Do not generate UCS flows for that class.
> `crates/integrations/connector-integration/src/authenticator_connectors/plaid.rs` is bank-account
> linking, not 3DS — it is a mechanism-C connector (see `pattern_client_authentication_token.md`).

**🎯 GENERIC PATTERN FILE FOR ANY NEW CONNECTOR**

This document provides comprehensive, reusable patterns for implementing the ServerSessionAuthenticationToken flow in **ANY** payment connector within the UCS (Universal Connector Service) system. These patterns are extracted from successful connector implementations (Paytm, Nuvei) and can be consumed by AI to generate consistent, production-ready ServerSessionAuthenticationToken flow code for any payment gateway.

> **🏗️ UCS-Specific:** This pattern is tailored for UCS architecture using RouterDataV2, ConnectorIntegrationV2, and domain_types. The ServerSessionAuthenticationToken flow is used to initiate a payment session and obtain a session token that can be used in subsequent authorization calls.

## 🚀 Quick Start Guide

To implement a new connector ServerSessionAuthenticationToken flow using these patterns:

1. **Choose Your Pattern**: Use [Modern Macro-Based Pattern](#modern-macro-based-pattern-recommended) for 95% of connectors
2. **Enable Session Token Flow**: Implement `ValidationTrait::should_do_session_token(&self, connector_feature_data: Option<&Secret<String>>)` returning `true`
3. **Replace Placeholders**: Follow the [Placeholder Reference Guide](#placeholder-reference-guide)
4. **Select Components**: Choose auth type, request format, and amount converter based on your connector's API
5. **Follow Checklist**: Use the [Integration Checklist](#integration-checklist) to ensure completeness

### Example: Implementing "NewPayment" Connector ServerSessionAuthenticationToken Flow

```bash
# Replace placeholders:
{ConnectorName} → NewPayment
{connector_name} → new_payment
{AmountType} → the unit matching the vendor's wire format: MinorUnit (1250), StringMinorUnit ("1250"),
#                StringMajorUnit ("12.50"), FloatMajorUnit (12.50), StringTwoDecimalUnit ("12.50", zero-padded).
#                All five in common_utils/src/types.rs. Do not default to StringMinorUnit.
{content_type} → "application/json" (if API uses JSON)
{session_token_endpoint} → "v1/session-token" (your API endpoint)
```

**✅ Result**: Complete, production-ready connector ServerSessionAuthenticationToken flow implementation in ~20 minutes

## Table of Contents

1. [Overview](#overview)
2. [ServerSessionAuthenticationToken Flow Implementation Analysis](#serversessionauthenticationtoken-flow-implementation-analysis)
   - [Where the session token actually goes](#where-the-session-token-actually-goes)
3. [Data Types](#data-types)
4. [Modern Macro-Based Pattern (Recommended)](#modern-macro-based-pattern-recommended)
5. [ValidationTrait Implementation](#validationtrait-implementation)
6. [Request/Response Format Variations](#requestresponse-format-variations)
7. [Session Token Usage Patterns](#session-token-usage-patterns)
8. [Error Handling Patterns](#error-handling-patterns)
9. [Testing Patterns](#testing-patterns)
10. [Integration Checklist](#integration-checklist)
11. [Change Log](#change-log)

## Overview

The ServerSessionAuthenticationToken flow is a pre-authorization step that:
1. Receives session token creation requests from the router
2. Transforms them to connector-specific format
3. Sends requests to the payment gateway to initiate a session
4. Processes responses and extracts session tokens
5. Returns standardized responses containing the session token for use in authorization

### Key Components:
- **Main Connector File**: Implements traits and flow logic
- **Transformers File**: Handles request/response data transformations
- **ValidationTrait**: Enables the ServerSessionAuthenticationToken flow
- **Authentication**: Manages API credentials and headers
- **Error Handling**: Processes and maps error responses
- **Session Token Carrier**: returns the token in `ServerSessionAuthenticationTokenResponseData`; the caller folds it onto the next payment request (see "Where the session token actually goes")

### Flow Sequence:
```
┌─────────────┐     ┌──────────────────┐     ┌─────────────┐
│   Router    │────▶│ ServerSessionAuthenticationToken│────▶│  Connector  │
│             │     │     Flow         │     │   Session   │
└─────────────┘     └──────────────────┘     │   Endpoint  │
                                              └──────┬──────┘
                                                     │
                                              ┌──────▼──────┐
                                              │   Returns   │
                                              │Session Token│
                                              └──────┬──────┘
                                                     │
┌─────────────┐     ┌──────────────────┐     ┌──────▼──────┐
│   Router    │◀────│     Authorize    │◀────│  Uses Token │
│             │     │     Flow         │     │  in Request │
└─────────────┘     └──────────────────┘     └─────────────┘
```

## ServerSessionAuthenticationToken Flow Implementation Analysis

**Re-derive this roster; do not trust a printed one.**

```bash
rg -l "flow: ServerSessionAuthenticationToken," crates/integrations/connector-integration/src/ \
  | grep -v /macros.rs
```

`connectors/macros.rs` is the macro *definition* file and always matches — exclude it. Prefer
`flow: <marker>,` over `flow_name: <marker>` — the latter misses connectors that hand-write the
`ConnectorIntegrationV2` impl instead of calling `macro_connector_implementation!`.

### Full implementations at HEAD — 5 connectors

All under `crates/integrations/connector-integration/src/connectors/`:

| Connector | Notes |
|---|---|
| `authorizedotnet` | Request/response types `AuthorizedotnetSdkSessionTokenRequest` / `...Response`. |
| `grabpay` | `GrabpayServerSessionAuthenticationTokenRequest` / `...Response`. Also overrides `next_authentication_step` (see the 3DS dispatch note in the flow-implementation guide) — the two mechanisms are independent. |
| `nuvei` | Session-based authentication for card payments: `getSessionToken.do` before `payment.do`, checksum auth. |
| `paytm` | Multi-step UPI flow with AES-CBC signature; `initiateTransaction` before `processTransaction`. |
| `payu` | `PayuSessionTokenRequest` / `PayuSessionTokenResponse`. |

Every other connector carries only the one-line marker impl
`impl ... connector_types::ServerSessionAuthentication for <Connector><T> {}` to satisfy the
`ConnectorServiceTrait` bound; the default `ConnectorIntegrationV2` bodies apply.

> **The previous "2 full / 75 stub of 77" roll-call has been deleted rather than refreshed.** It was
> wrong in both directions: it named `AuthorizeDotNet`, `PayU` and `Revolut` as stubs (two of the
> three are now full implementations), and it named connectors that no longer exist. Derive the stub
> set as "every connector not in the table above".

### Where the session token actually goes

This is the correction that matters most. The flow does **not** write
`PaymentFlowData.session_token` — its `resource_common_data` is `MerchantAuthenticationFlowData`,
which has no such field. The carrier chain is:

1. The connector's response transformer returns
   `Ok(ServerSessionAuthenticationTokenResponseData { session_token })` and passes
   `resource_common_data` through untouched. Exemplar:
   `impl TryFrom<ResponseRouterData<NuveiSessionTokenResponse, Self>>` in
   `crates/integrations/connector-integration/src/connectors/nuvei/transformers.rs` — it sets only
   `response:` and spreads `..router_data.clone()`.
2. That becomes gRPC
   `MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse.session_token`
   (`crates/types-traits/grpc-api-types/proto/services.proto`, rpc
   `CreateServerSessionAuthenticationToken`).
3. `crates/internal/composite-service/src/payments.rs` orchestrates the leg: it calls
   `should_do_session_token(...)` on the connector, and when the payment request has no
   session token of its own, invokes
   `merchant_authentication_service.create_server_session_authentication_token(...)`.
4. `pub fn get_session_token(session_token_from_request, session_token_response)` in
   `crates/internal/composite-service/src/utils.rs` folds the result onto the outgoing
   `PaymentServiceAuthorizeRequest.session_token` — **a token supplied on the request wins**; the
   freshly minted one is only the fallback.
5. `PaymentFlowData.session_token: Option<String>` is populated from that gRPC field
   (`crates/types-traits/domain_types/src/types.rs`), and the Authorize transformer reads it via
   `router_data.resource_common_data.session_token` or `get_session_token()`.

So: **the SSAT transformer writes the response; the Authorize transformer reads `PaymentFlowData`.**
Those are two different `resource_common_data` types, in two different gRPC calls.

### Other observations

- **Most common pattern**: POST with a JSON request body.
- **Most common auth**: custom signature / checksum, computed in the transformer.
- The flow is gated by `ValidationTrait::should_do_session_token` (see below) — without the
  override it never runs.

## Data Types

Read these from source before writing any transformer — none of the three derives `Default`.

### Flow marker

`pub struct ServerSessionAuthenticationToken;` — `crates/types-traits/domain_types/src/connector_flow.rs`
(plus its `FlowName::ServerSessionAuthenticationToken` entry in the same file). It is a bare unit
struct: it declares no associated `Request`/`Response` types, and there is no `ConnectorFlow` trait
anywhere in the tree.

### Trait binding

```rust
// crates/types-traits/interfaces/src/connector_types.rs — `pub trait ServerSessionAuthentication`
pub trait ServerSessionAuthentication:
    ConnectorIntegrationV2<
    connector_flow::ServerSessionAuthenticationToken,
    MerchantAuthenticationFlowData,                      // NOT PaymentFlowData
    ServerSessionAuthenticationTokenRequestData,
    ServerSessionAuthenticationTokenResponseData,
>
{
}
```

`ServerSessionAuthentication` is a supertrait of `ConnectorServiceTrait` (same file), so every
payment connector needs at least the empty marker impl.

### Request data

```rust
// crates/types-traits/domain_types/src/connector_types.rs
//   `pub struct ServerSessionAuthenticationTokenRequestData`
#[derive(Debug, Clone)]
pub struct ServerSessionAuthenticationTokenRequestData {
    pub amount: MinorUnit,
    pub currency: Currency,
    pub browser_info: Option<BrowserInformation>,
    pub customer_id: Option<common_utils::id_type::CustomerId>,
    pub address: Option<payment_address::PaymentAddress>,
}
```

Five fields, no `Default`. Its `impl` block carries the accessors you should prefer over manual
`ok_or`: `get_browser_info()`, `get_customer_id()`, `get_optional_billing()`,
`get_optional_billing_first_name()`, `get_optional_billing_last_name()`, and the shipping
equivalents — grep `impl ServerSessionAuthenticationTokenRequestData` for the current list.

### Response data

```rust
#[derive(Debug, Clone)]
pub struct ServerSessionAuthenticationTokenResponseData {
    pub session_token: String,
}
```

One field, a plain `String` (not `Secret`, not `Option`). If the connector omits the token, that is
an error path — do not synthesise an empty string.

### Resource common data

`pub struct MerchantAuthenticationFlowData` —
`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`. Fields: `merchant_id`,
`connectors`, `connector_request_reference_id`, `test_mode`, `return_url`, `connector_feature_data`,
`order_details`, `merchant_request_id`, plus five observability fields. One inherent method:
`get_return_url()`.

Its doc-comment states the design intent: *"This type deliberately omits payment-specific fields
(`payment_id`, `attempt_id`, `status`, `payment_method`, `address`, `amount`, etc.) because
merchant-authentication flows have no payment identity."* Note `amount`/`currency` still reach the
flow — on the **request**, not on `resource_common_data`.

### gRPC surface

`service MerchantAuthenticationService` → `rpc CreateServerSessionAuthenticationToken` in
`crates/types-traits/grpc-api-types/proto/services.proto`. This is a different service from
`PaymentService`; the leg is a separate RPC, not a sub-step of Authorize.

## Modern Macro-Based Pattern (Recommended)

This is the current recommended approach using the macro framework for maximum code reuse and consistency.

### File Structure Template

```
connector-service/crates/integrations/connector-integration/src/connectors/
├── {connector_name}.rs           # Main connector implementation
└── {connector_name}/
    └── transformers.rs           # Data transformation logic
```

### Main Connector File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}.rs

pub mod transformers;

use common_utils::{errors::CustomResult, ext_traits::ByteSliceExt};
use domain_types::{
    connector_flow::{
        Accept, Authorize, Capture, CreateOrder, ServerSessionAuthenticationToken, DefendDispute, PSync, RSync,
        Refund, RepeatPayment, SetupMandate, SubmitEvidence, Void,
    },
    connector_types::{
        AcceptDisputeData, DisputeDefendData, DisputeFlowData, DisputeResponseData,
        PaymentCreateOrderData, PaymentCreateOrderResponse, PaymentFlowData, PaymentVoidData,
        // ServerSessionAuthenticationToken binds MerchantAuthenticationFlowData, imported below
        // from its own module — it is NOT in domain_types::connector_types.
        PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData, PaymentsSyncData,
        RefundFlowData, RefundSyncData, RefundsData, RefundsResponseData, RepeatPaymentData,
        ResponseId, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData, SetupMandateRequestData,
        SubmitEvidenceData,
    },
    errors::{self, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
    types::Connectors,
};
use error_stack::ResultExt;
use hyperswitch_masking::{Mask, Maskable};
// The event type is `common_utils::events::Event`; `interfaces::events::connector_api_logs::ConnectorEvent`
// is not the type the connector traits take. Import the module and write `events::Event`,
// exactly as connectors/travelhub.rs does.
use common_utils::events;
use interfaces::{
    api::ConnectorCommon, connector_integration_v2::ConnectorIntegrationV2, connector_types,
};
use serde::Serialize;
use transformers::{
    {ConnectorName}SessionTokenRequest, {ConnectorName}SessionTokenResponse,
    {ConnectorName}AuthorizeRequest, {ConnectorName}AuthorizeResponse,
    {ConnectorName}ErrorResponse,
};

use super::macros;
use crate::types::ResponseRouterData;

// Set up connector using macros with all framework integrations
macros::create_all_prerequisites!(
    connector_name: {ConnectorName},
    generic_type: T,
    api: [
        (
            flow: ServerSessionAuthenticationToken,
            request_body: {ConnectorName}SessionTokenRequest,
            response_body: {ConnectorName}SessionTokenResponse,
            router_data: RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ),
        (
            flow: Authorize,
            request_body: {ConnectorName}AuthorizeRequest<T>,
            response_body: {ConnectorName}AuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        // Add other flows as needed...
    ],
    amount_converters: [
        // Choose appropriate amount converter based on connector requirements
        // Pick the unit that matches the vendor's documented wire format. FIVE exist in
        // common_utils/src/types.rs: MinorUnit(:170), StringMinorUnit(:305), FloatMajorUnit(:336),
        // StringMajorUnit(:374), StringTwoDecimalUnit(:443). Do NOT default to StringMinorUnit —
        // on HEAD the split is StringMajorUnit 24 / FloatMajorUnit 22 / MinorUnit 11 / StringMinorUnit 8.
        amount_converter: {AmountUnit}
    ],
    member_functions: {
        pub fn build_headers<F, FCD, Req, Res>(
            &self,
            req: &RouterDataV2<F, FCD, Req, Res>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            let mut header = vec![(
                "Content-Type".to_string(),
                "{content_type}".to_string().into(),
            )];
            let mut auth_header = self.get_auth_header(&req.connector_config)?;
            header.append(&mut auth_header);
            Ok(header)
        }

        pub fn connector_base_url_payments<'a, F, Req, Res>(
            &self,
            req: &'a RouterDataV2<F, PaymentFlowData, Req, Res>,
        ) -> &'a str {
            &req.resource_common_data.connectors.{connector_name}.base_url
        }

        // REQUIRED for ServerSessionAuthenticationToken: its resource_common_data is
        // MerchantAuthenticationFlowData, so `connector_base_url_payments` does not typecheck
        // for this flow. Real exemplars: `connector_base_url_merchant_auth` in
        // crates/integrations/connector-integration/src/connectors/{paytm,nuvei,volt}.rs.
        pub fn connector_base_url_merchant_auth<F, Req, Res>(
            &self,
            req: &RouterDataV2<F, MerchantAuthenticationFlowData, Req, Res>,
        ) -> String {
            req.resource_common_data.connectors.{connector_name}.base_url.to_string()
        }
    }
);

// CRITICAL: Implement ValidationTrait to enable ServerSessionAuthenticationToken flow
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for {ConnectorName}<T>
{
    // Real signature — takes the connector_feature_data blob so a connector can decide per-merchant.
    // Verify: `pub trait ValidationTrait` in crates/types-traits/interfaces/src/connector_types.rs.
    fn should_do_session_token(
        &self,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> bool {
        true // Enable ServerSessionAuthenticationToken flow
    }

    fn should_do_order_create(&self) -> bool {
        false // Set to true if connector requires separate order creation
    }
}

// Implement ConnectorCommon trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    ConnectorCommon for {ConnectorName}<T>
{
    fn id(&self) -> &'static str {
        "{connector_name}"
    }

    fn get_currency_unit(&self) -> common_enums::CurrencyUnit {
        common_enums::CurrencyUnit::{Major|Minor}
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        &connectors.{connector_name}.base_url
    }

    fn get_auth_header(
        &self,
        auth_type: &ConnectorSpecificConfig,
    ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
        let auth = transformers::{ConnectorName}AuthType::try_from(auth_type)
            .change_context(errors::IntegrationError::FailedToObtainAuthType { context: Default::default() })?;

        Ok(vec![(
            "Authorization".to_string(),
            format!("Bearer {}", auth.api_key.peek()).into_masked(),
        )])
    }

    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, errors::ConnectorError> {
        let response: {ConnectorName}ErrorResponse = if res.response.is_empty() {
            {ConnectorName}ErrorResponse::default()
        } else {
            res.response
                .parse_struct("ErrorResponse")
                .change_context(errors::ConnectorError::ResponseDeserializationFailed { context: Default::default() })?
        };

        // `with_error_response_body!` is a crate macro: `use crate::with_error_response_body;`
        // (definition: crates/integrations/connector-integration/src/utils.rs). It expands to
        // `if let Some(body) = event_builder { body.set_connector_response(&response); }` — there is no
        // `set_error_response_body` method on `events::Event`.
        with_error_response_body!(event_builder, response);

        Ok(ErrorResponse {
            status_code: res.status_code,
            // `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` come from `common_utils::consts`
            // (crates/common/common_utils/src/consts.rs). Import them:
            //     use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
            // Real connectors reference them 497 times across 90 files; never `unwrap_or_default()` an error code/message —
            // an empty string in a log is indistinguishable from "the connector sent nothing".
            code: response.error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response.error_message.unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.error_description,
            attempt_status: None,
            connector_transaction_id: response.transaction_id,
            ..Default::default()
        })
    }
}

// Implement ServerSessionAuthenticationToken flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}SessionTokenRequest),
    curl_response: {ConnectorName}SessionTokenResponse,
    flow_name: ServerSessionAuthenticationToken,
    resource_common_data: MerchantAuthenticationFlowData,
    flow_request: ServerSessionAuthenticationTokenRequestData,
    flow_response: ServerSessionAuthenticationTokenResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_merchant_auth(req);
            Ok(format!("{base_url}/{session_token_endpoint}"))
        }
    }
);

// Implement Authorize flow using macro framework
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: {ConnectorName},
    curl_request: Json({ConnectorName}AuthorizeRequest<T>),
    curl_response: {ConnectorName}AuthorizeResponse,
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
        ) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ) -> CustomResult<String, IntegrationError> {
            let base_url = self.connector_base_url_payments(req);
            Ok(format!("{base_url}/{authorize_endpoint}"))
        }
    }
);
```

### Transformers File Pattern

```rust
// File: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

// All five unit types live in common_utils::types; import the one matching the vendor's wire format.
use common_utils::types::{MinorUnit, StringMinorUnit};
use domain_types::{
    connector_flow::{Authorize, ServerSessionAuthenticationToken},
    connector_types::{
        PaymentFlowData, PaymentsAuthorizeData, PaymentsResponseData,
        ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData, ResponseId,
    },
    errors::{self, IntegrationError},
    merchant_authentication_flow_data::MerchantAuthenticationFlowData,
    payment_method_data::PaymentMethodDataTypes,
    router_data::{ConnectorSpecificConfig, ErrorResponse},
    router_data_v2::RouterDataV2,
};
use error_stack::ResultExt;
use hyperswitch_masking::{ExposeInterface, Secret, PeekInterface};
use serde::{Deserialize, Serialize};

use crate::types::ResponseRouterData;

// Authentication Type Definition
#[derive(Debug)]
pub struct {ConnectorName}AuthType {
    pub api_key: Secret<String>,
    pub api_secret: Option<Secret<String>>,
    // Add other auth fields as needed
}

// Auth is read from `ConnectorSpecificConfig`, NOT from a `connector_auth_type` field —
// `RouterDataV2` lost `connector_auth_type` on 2026-03-14 (a7a696c3a); the field is now
// `req.connector_config: ConnectorSpecificConfig` (domain_types/src/router_data_v2.rs).
// `ConnectorSpecificConfig` has ONE struct variant PER CONNECTOR (domain_types/src/router_data.rs),
// not generic HeaderKey/BodyKey/SignatureKey variants — add your connector's variant there and
// match on it. Exemplar: connectors/travelhub/transformers.rs and connectors/volt/transformers.rs.
impl TryFrom<&ConnectorSpecificConfig> for {ConnectorName}AuthType {
    type Error = IntegrationError;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        // ONE arm per connector: your connector has exactly one `ConnectorSpecificConfig`
        // variant, so destructure exactly the fields that variant declares. Optional
        // credentials are `Option<Secret<String>>` *in the variant itself* — do not try to
        // express "api_key only" vs "api_key + secret" as separate match arms; the second
        // arm would be unreachable (E0001-class dead code).
        match auth_type {
            ConnectorSpecificConfig::{ConnectorName} {
                api_key,
                api_secret,
                ..
            } => Ok(Self {
                api_key: api_key.to_owned(),
                api_secret: api_secret.to_owned(),
            }),
            _ => Err(IntegrationError::FailedToObtainAuthType { context: Default::default() }),
        }
    }
}

// ================================
// Session Token Flow
// ================================

// Session Token Request Structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SessionTokenRequest {
    // Common fields for session token requests
    pub merchant_id: Secret<String>,
    pub client_request_id: String,
    pub timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<{AmountType}>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    // Add connector-specific fields
}

// Session Token Response Structure
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}SessionTokenResponse {
    pub session_token: Option<String>,
    pub status: {ConnectorName}SessionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum {ConnectorName}SessionStatus {
    Success,
    Failed,
    Error,
    #[serde(other)]
    Unknown,
}

// Session Token Request Transformation
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
            T,
        >,
    > for {ConnectorName}SessionTokenRequest
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_config)?;

        // Convert amount if needed
        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::AmountConversionFailed { context: Default::default() })?;

        Ok(Self {
            merchant_id: auth.api_key,
            client_request_id: router_data.resource_common_data.connector_request_reference_id.clone(),
            timestamp: chrono::Utc::now().timestamp().to_string(),
            amount: Some(amount),
            currency: Some(router_data.request.currency.to_string()),
        })
    }
}

// Session Token Response Transformation
impl TryFrom<ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>>
    for RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<ConnectorError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // NOTE — mechanism C: `resource_common_data` here is `MerchantAuthenticationFlowData`.
        // It has NO `status`, NO `session_token`, NO `access_token`. Never construct it in this
        // transformer; pass it through with `..router_data.clone()`. Any `PaymentFlowData { .. }`
        // literal in this impl is a copy-paste from a payment flow and will not compile (E0609 /
        // E0308). Compare `impl TryFrom<ResponseRouterData<NuveiSessionTokenResponse, Self>>` in
        // crates/integrations/connector-integration/src/connectors/nuvei/transformers.rs, which
        // sets only `response`.

        // Check for error status
        if matches!(response.status, {ConnectorName}SessionStatus::Error | {ConnectorName}SessionStatus::Failed) {
            // In-band 2xx failure: the connector answered 200 with a terminal Error/Failed
            // status in the body, so `Failure` here is derived from the connector's own status
            // enum, not from the HTTP code. Never set `Failure` on a path where the body has
            // not proven the attempt failed. See `domain_types::utils::is_payment_failure`
            // (domain_types/src/utils.rs).
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: response.error_code.clone().unwrap_or_else(|| NO_ERROR_CODE.to_string()),
                    message: response.error_message.clone().unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.error_message.clone(),
                    status_code: item.http_code,
                    // `FlowStatus` is `domain_types::router_data::FlowStatus`:
                    //     use domain_types::router_data::FlowStatus;
                    // Variants: Payment(AttemptStatus) | Refund(RefundStatus) | Dispute(DisputeStatus) |
                    // Payout(PayoutStatus). Pick the one matching THIS flow.
                    attempt_status: Some(FlowStatus::Payment(common_enums::AttemptStatus::Failure)),
                    connector_transaction_id: None,
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        // Extract session token
        let session_token = response
            .session_token
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: Default::default(),
            })?;

        // Return the token in the RESPONSE. The caller carries it onto the next payment request;
        // this flow cannot and must not write it onto resource_common_data.
        Ok(Self {
            response: Ok(ServerSessionAuthenticationTokenResponseData {
                session_token,
            }),
            ..router_data.clone()
        })
    }
}

// ================================
// Authorization Flow
// ================================

// Authorization Request Structure
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct {ConnectorName}AuthorizeRequest<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub session_token: String,
    pub merchant_id: Secret<String>,
    pub amount: {AmountType},
    pub currency: String,
    pub payment_method: {ConnectorName}PaymentMethod<T>,
    pub reference: String,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum {ConnectorName}PaymentMethod<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    Card({ConnectorName}Card<T>),
}

#[derive(Debug, Serialize)]
pub struct {ConnectorName}Card<
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize,
> {
    pub number: domain_types::payment_method_data::RawCardNumber<T>,
    pub exp_month: Secret<String>,
    pub exp_year: Secret<String>,
    pub cvc: Secret<String>,
}

// Authorization Request Transformation
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    TryFrom<
        {ConnectorName}RouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    > for {ConnectorName}AuthorizeRequest<T>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: {ConnectorName}RouterData<
            RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
            T,
        >,
    ) -> Result<Self, Self::Error> {
        let router_data = &item.router_data;
        let auth = {ConnectorName}AuthType::try_from(&router_data.connector_config)?;

        // Extract session token from PaymentFlowData
        let session_token = router_data
            .resource_common_data
            .session_token
            .clone()
            .ok_or(IntegrationError::MissingRequiredField {
                field_name: "session_token",
                context: Default::default(),
            })?;

        // Convert amount
        let amount = item
            .connector
            .amount_converter
            .convert(router_data.request.amount, router_data.request.currency)
            .change_context(IntegrationError::AmountConversionFailed { context: Default::default() })?;

        // Build payment method
        let payment_method = match &router_data.request.payment_method_data {
            domain_types::payment_method_data::PaymentMethodData::Card(card_data) => {
                {ConnectorName}PaymentMethod::Card({ConnectorName}Card {
                    number: card_data.card_number.clone(),
                    exp_month: card_data.card_exp_month.clone(),
                    exp_year: card_data.card_exp_year.clone(),
                    cvc: card_data.card_cvc.clone(),
                })
            }
            _ => return Err(IntegrationError::NotImplemented("Payment method not supported".to_string(), Default::default()).into()),
        };

        Ok(Self {
            session_token,
            merchant_id: auth.api_key,
            amount,
            currency: router_data.request.currency.to_string(),
            payment_method,
            reference: router_data.resource_common_data.connector_request_reference_id.clone(),
        })
    }
}

// Helper struct for router data transformation
pub struct {ConnectorName}RouterData<T, U> {
    pub router_data: T,
    pub connector: U,
}

impl<T, U> TryFrom<(T, U)> for {ConnectorName}RouterData<T, U> {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from((router_data, connector): (T, U)) -> Result<Self, Self::Error> {
        Ok(Self {
            router_data,
            connector,
        })
    }
}

// Error Response Structure
#[derive(Debug, Deserialize, Default)]
pub struct {ConnectorName}ErrorResponse {
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_description: Option<String>,
    pub transaction_id: Option<String>,
}
```

## ValidationTrait Implementation

**CRITICAL**: To enable the ServerSessionAuthenticationToken flow, you MUST override `ValidationTrait::should_do_session_token` to return `true`. The trait default is `false`, so without the override the composite service never issues the leg (`crates/internal/composite-service/src/payments.rs` reads `connector_data.connector.should_do_session_token(payload.connector_feature_data())`). The parameter is **`Option<&hyperswitch_masking::Secret<String>>`**, not `()`:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for {ConnectorName}<T>
{
    // Real signature — takes the connector_feature_data blob so a connector can decide per-merchant.
    // Verify: `pub trait ValidationTrait` in crates/types-traits/interfaces/src/connector_types.rs.
    fn should_do_session_token(
        &self,
        _connector_feature_data: Option<&hyperswitch_masking::Secret<String>>,
    ) -> bool {
        true // Enable ServerSessionAuthenticationToken flow
    }

    fn should_do_order_create(&self) -> bool {
        false // Set to true if connector requires separate order creation
    }
}
```

This trait tells the router to execute the ServerSessionAuthenticationToken flow before the Authorize flow.

## Request/Response Format Variations

### Simple Session Token Pattern (Nuvei-style)

For connectors that only need basic merchant authentication to get a session token:

```rust
// Request
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenRequest {
    pub merchant_id: Secret<String>,
    pub merchant_site_id: Secret<String>,
    pub client_request_id: String,
    pub time_stamp: DateTime<YYYYMMDDHHmmss>,
    pub checksum: String,
}

// Response
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NuveiSessionTokenResponse {
    pub session_token: Option<String>,
    pub status: NuveiPaymentStatus,
    pub err_code: Option<i32>,
    pub reason: Option<String>,
}
```

### Complex Session Initiation Pattern (Paytm-style)

For connectors that require full payment details to initiate a session:

```rust
// Request with signature
#[derive(Debug, Serialize)]
pub struct PaytmInitiateTxnRequest {
    pub head: PaytmRequestHeader,
    pub body: PaytmInitiateReqBody,
}

#[derive(Debug, Serialize)]
pub struct PaytmRequestHeader {
    pub client_id: Option<Secret<String>>,
    pub version: String,
    pub request_timestamp: String,
    pub channel_id: Option<String>,
    pub signature: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct PaytmInitiateReqBody {
    pub request_type: String,
    pub mid: Secret<String>,
    pub order_id: String,
    pub website_name: Secret<String>,
    pub txn_amount: PaytmAmount,
    pub user_info: PaytmUserInfo,
    pub callback_url: String,
}

// Response
#[derive(Debug, Deserialize)]
pub struct PaytmInitiateTxnResponse {
    pub head: PaytmRespHead,
    pub body: PaytmResBodyTypes,
}
```

## Session Token Usage Patterns

### Pattern 1: Token in Request Body (Nuvei-style)

```rust
#[derive(Debug, Serialize)]
pub struct NuveiPaymentRequest<T> {
    pub session_token: Option<String>,
    pub merchant_id: Secret<String>,
    pub amount: StringMajorUnit,
    pub currency: Currency,
    pub payment_option: NuveiPaymentOption<T>,
}

// In TryFrom for Authorize request:
let session_token = router_data
    .resource_common_data
    .session_token
    .clone()
    .ok_or(IntegrationError::MissingRequiredField {
        field_name: "session_token",
        context: Default::default(),
    })?;
```

### Pattern 2: Token in Headers (Alternative)

```rust
fn get_headers(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let session_token = req.resource_common_data.get_session_token()?;
    let mut headers = vec![
        ("X-Session-Token".to_string(), session_token.into_masked()),
        ("Content-Type".to_string(), "application/json".into()),
    ];
    Ok(headers)
}
```

### Pattern 3: Token in URL Query Parameters (Alternative)

```rust
fn get_url(
    &self,
    req: &RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let session_token = req.resource_common_data.get_session_token()?;
    let base_url = self.connector_base_url_payments(req);
    Ok(format!("{base_url}/payment?sessionToken={}", session_token))
}
```

## Error Handling Patterns

### Session Token Specific Error Handling

```rust
impl TryFrom<ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>>
    for RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData>
{
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(
        item: ResponseRouterData<{ConnectorName}SessionTokenResponse, Self>,
    ) -> Result<Self, Self::Error> {
        let response = &item.response;
        let router_data = &item.router_data;

        // Handle session token specific errors
        if let Some(error_code) = &response.error_code {
            // Map only the codes this connector documents. An unrecognised code must NOT be
            // forced to a terminal status: `ErrorResponse.attempt_status` is
            // `Option<FlowStatus>` (router_data.rs) and stays `None` when unknown.
            // Exemplars: connectors/noon.rs, connectors/flywire.rs.
            let mapped_status = match error_code.as_str() {
                "INVALID_MERCHANT" => Some(common_enums::AttemptStatus::AuthorizationFailed),
                "RATE_LIMIT_EXCEEDED" => Some(common_enums::AttemptStatus::Pending),
                "SESSION_EXPIRED" => Some(common_enums::AttemptStatus::Failure),
                _ => None,
            };

            // No `resource_common_data:` override — `MerchantAuthenticationFlowData` has no
            // `status` field to set. The mapped status travels only on `ErrorResponse.attempt_status`,
            // which the caller applies to the payment attempt.
            return Ok(Self {
                response: Err(ErrorResponse {
                    code: error_code.clone(),
                    message: response.error_message.clone().unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
                    reason: response.error_message.clone(),
                    status_code: item.http_code,
                    attempt_status: mapped_status.map(FlowStatus::Payment),
                    connector_transaction_id: None,
                    ..Default::default()
                }),
                ..router_data.clone()
            });
        }

        // Success case...
    }
}
```

## Testing Patterns

### Unit Test Structure for ServerSessionAuthenticationToken Flow

```rust
#[cfg(test)]
mod session_token_tests {
    use super::*;

    #[test]
    fn test_session_token_request_transformation() {
        let router_data = create_test_session_token_router_data();
        let connector_req = {ConnectorName}SessionTokenRequest::try_from(&router_data);

        assert!(connector_req.is_ok());
        let req = connector_req.unwrap();
        assert!(!req.client_request_id.is_empty());
    }

    #[test]
    fn test_session_token_response_transformation_success() {
        let response = {ConnectorName}SessionTokenResponse {
            session_token: Some("test_session_token_123".to_string()),
            status: {ConnectorName}SessionStatus::Success,
            error_code: None,
            error_message: None,
        };

        let router_data = create_test_session_token_router_data();
        let response_router_data = ResponseRouterData {
            response,
            router_data: router_data,
            http_code: 200,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        // Assert on the RESPONSE, not on resource_common_data — `MerchantAuthenticationFlowData`
        // has no `session_token` field.
        let router_data_result = result.unwrap();
        let response = router_data_result.response.expect("expected Ok response");
        assert_eq!(response.session_token, "test_session_token_123");
    }

    #[test]
    fn test_session_token_response_transformation_failure() {
        let response = {ConnectorName}SessionTokenResponse {
            session_token: None,
            status: {ConnectorName}SessionStatus::Error,
            error_code: Some("INVALID_MERCHANT".to_string()),
            error_message: Some("Merchant not found".to_string()),
        };

        let router_data = create_test_session_token_router_data();
        let response_router_data = ResponseRouterData {
            response,
            router_data: router_data,
            http_code: 400,
        };

        let result = RouterDataV2::try_from(response_router_data);
        assert!(result.is_ok());

        let router_data_result = result.unwrap();
        assert!(router_data_result.response.is_err());
    }

    fn create_test_session_token_router_data() -> RouterDataV2<ServerSessionAuthenticationToken, MerchantAuthenticationFlowData, ServerSessionAuthenticationTokenRequestData, ServerSessionAuthenticationTokenResponseData> {
        // NOTE: neither `RouterDataV2`, `MerchantAuthenticationFlowData` nor
        // `ServerSessionAuthenticationTokenRequestData` derives `Default` — `..Default::default()`
        // does NOT compile here. Every field must be named. Read the two structs before writing
        // this fixture:
        //   crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs
        //   crates/types-traits/domain_types/src/connector_types.rs
        //     (`pub struct ServerSessionAuthenticationTokenRequestData`)
        RouterDataV2 {
            flow: std::marker::PhantomData,
            resource_common_data: MerchantAuthenticationFlowData {
                merchant_id: /* common_utils::id_type::MerchantId */ test_merchant_id(),
                connectors: std::sync::Arc::new(test_connectors()),
                connector_request_reference_id: "test_order_123".to_string(),
                test_mode: Some(true),
                return_url: None,
                connector_feature_data: None,
                order_details: None,
                merchant_request_id: None,
                raw_connector_response: None,
                typed_connector_response: None,
                raw_connector_request: None,
                typed_connector_request: None,
                connector_response_headers: None,
            },
            connector_config: test_connector_config(),
            request: ServerSessionAuthenticationTokenRequestData {
                amount: MinorUnit::new(1000),
                currency: common_enums::Currency::USD,
                browser_info: None,
                customer_id: None,
                address: None,
            },
            response: Ok(ServerSessionAuthenticationTokenResponseData {
                session_token: String::new(),
            }),
        }
    }
}
```

## Integration Checklist

### Pre-Implementation Checklist

- [ ] **API Documentation Review**
  - [ ] Understand connector's session token API endpoint
  - [ ] Review authentication requirements for session token request
  - [ ] Identify required/optional fields for session token request
  - [ ] Understand session token expiration behavior
  - [ ] Review how session token is used in authorization

- [ ] **Flow Requirements**
  - [ ] Determine if connector requires ServerSessionAuthenticationToken flow
  - [ ] Understand sequence: Session Token → Authorize
  - [ ] Check if session token can be reused across multiple requests
  - [ ] Identify session token expiration time

### Implementation Checklist

- [ ] **Main Connector Implementation**
  - [ ] Add `ServerSessionAuthenticationToken` to connector_flow imports
  - [ ] Add `ServerSessionAuthenticationTokenRequestData` and `ServerSessionAuthenticationTokenResponseData` to connector_types imports
  - [ ] Import session token request/response types from transformers
  - [ ] Override `ValidationTrait::should_do_session_token(&self, Option<&Secret<String>>)` to return `true`
  - [ ] Add ServerSessionAuthenticationToken flow to `macros::create_all_prerequisites!`
  - [ ] Implement ServerSessionAuthenticationToken flow with `macros::macro_connector_implementation!`
  - [ ] Add Source Verification stub for ServerSessionAuthenticationToken flow

- [ ] **Transformers Implementation**
  - [ ] Add `ServerSessionAuthenticationToken` to connector_flow imports
  - [ ] Add `ServerSessionAuthenticationTokenRequestData` and `ServerSessionAuthenticationTokenResponseData` to connector_types imports
  - [ ] Create session token request structure
  - [ ] Create session token response structure
  - [ ] Create session status enumeration
  - [ ] Implement session token request transformation (`TryFrom`)
  - [ ] Implement session token response transformation (`TryFrom`)
  - [ ] Return the token in `ServerSessionAuthenticationTokenResponseData` — do NOT construct a `resource_common_data` literal (`MerchantAuthenticationFlowData` has no `session_token`)
  - [ ] Extract and use session token in Authorize flow

### Testing Checklist

- [ ] **Unit Tests**
  - [ ] Test session token request transformation
  - [ ] Test session token response transformation (success)
  - [ ] Test session token response transformation (failure)
  - [ ] Assert the token on `router_data.response`, not on `resource_common_data`
  - [ ] Test session token retrieval in Authorize flow
  - [ ] Test error response handling

- [ ] **Integration Tests**
  - [ ] Test complete flow: ServerSessionAuthenticationToken → Authorize
  - [ ] Test session token expiration handling
  - [ ] Test error scenarios

### Validation Checklist

- [ ] **Code Quality**
  - [ ] Run `cargo build` and fix all errors
  - [ ] Run `cargo test` and ensure all tests pass
  - [ ] Run `cargo clippy` and fix warnings

- [ ] **Functionality Validation**
  - [ ] Test with sandbox/test credentials
  - [ ] Verify session token is received
  - [ ] Verify session token is used in authorization
  - [ ] Verify error handling works correctly

## Placeholder Reference Guide

**🔄 UNIVERSAL REPLACEMENT SYSTEM FOR SERVERSESSIONAUTHENTICATIONTOKEN FLOWS**

| Placeholder | Description | Example Values | When to Use |
|-------------|-------------|----------------|-------------|
| `{ConnectorName}` | Connector name in PascalCase | `Stripe`, `Nuvei`, `PayPal`, `NewPayment` | **Always required** - Used in struct names |
| `{connector_name}` | Connector name in snake_case | `stripe`, `nuvei`, `paypal`, `new_payment` | **Always required** - Used in config keys |
| `{AmountType}` | Amount type based on connector API | `MinorUnit`, `StringMinorUnit`, `StringMajorUnit`, `FloatMajorUnit`, `StringTwoDecimalUnit` | **Read the vendor spec and match its wire format** — no safe default |
| `{content_type}` | Request content type | `"application/json"`, `"application/x-www-form-urlencoded"` | **Based on API format** |
| `{session_token_endpoint}` | Session token API endpoint | `"v1/session-token"`, `"getSessionToken.do"` | **From API docs** |
| `{authorize_endpoint}` | Authorization API endpoint | `"v1/payments"`, `"payment.do"` | **From API docs** |
| `{Major\|Minor}` | Currency unit choice | `Major` or `Minor` | **Choose one** |

### Real-World Examples

**Example 1: Nuvei-style Connector**
```bash
{ConnectorName} → Nuvei
{connector_name} → nuvei
{AmountType} → StringMajorUnit
{content_type} → "application/json"
{session_token_endpoint} → "getSessionToken.do"
{authorize_endpoint} → "payment.do"
{Major|Minor} → Minor
```

**Example 2: Paytm-style Connector**
```bash
{ConnectorName} → Paytm
{connector_name} → paytm
{AmountType} → StringMajorUnit
{content_type} → "application/json"
{session_token_endpoint} → "theia/api/v1/initiateTransaction"
{authorize_endpoint} → "theia/api/v1/processTransaction"
{Major|Minor} → Minor
```

## Best Practices

1. **Enable via ValidationTrait**: Always override `should_do_session_token(&self, connector_feature_data: Option<&Secret<String>>)` to return `true` — the trait default is `false` and the flow is otherwise never dispatched

2. **Return the Token, Don't Store It**: the SSAT transformer sets only `response: Ok(ServerSessionAuthenticationTokenResponseData { session_token })`. The composite service (`composite-service/src/utils.rs::get_session_token`) folds it onto the next payment request, where it lands on `PaymentFlowData.session_token` for the Authorize transformer to read

3. **Handle Missing Token**: In Authorize flow, always check for the session token and return a clear error if missing

4. **Token Expiration**: Consider token expiration if the connector has time limits on session token validity

5. **Error Handling**: Implement specific error handling for session token failures vs authorization failures

6. **Testing**: Test the complete flow end-to-end: ServerSessionAuthenticationToken → Authorize

### Common Pitfalls to Avoid

- **Missing ValidationTrait**: without the `should_do_session_token` override the flow is never dispatched; and note the parameter is `Option<&Secret<String>>` — a zero-arg override does not compile
- **Trying to Store the Token**: writing `resource_common_data: PaymentFlowData { session_token: ..., .. }` in this flow. That is a copy-paste from a payment flow and does not compile — this flow's `resource_common_data` is `MerchantAuthenticationFlowData`
- **Not Using Token**: Authorize flow not extracting and using the session token
- **Wrong Status Mapping**: Session token responses should typically map to `Pending` status, not `Charged`
- **Error Propagation**: Not properly propagating session token errors to prevent authorization attempts

This pattern document provides a comprehensive template for implementing ServerSessionAuthenticationToken flows in payment connectors, ensuring consistency and completeness across all implementations.

## Change Log

| Version | Date | Change |
|---|---|---|
| 2.0.0 | 2026-09-07 | Mechanism-C correction pass against HEAD. **`PaymentFlowData` → `MerchantAuthenticationFlowData` at every site in this flow's generics and macro blocks** (~13 sites); the SSAT `resource_common_data` cannot hold a session token, so the "store the token in `PaymentFlowData.session_token`" instruction — repeated in the overview, the response transformer, the checklist, the best practices and the pitfalls — was replaced with the real carrier chain (response → gRPC `CreateServerSessionAuthenticationToken` → `composite-service/src/utils.rs::get_session_token` → `PaymentServiceAuthorizeRequest.session_token` → `PaymentFlowData.session_token`, request value winning over the minted one). Corrected `should_do_session_token` to take `Option<&Secret<String>>`. Added a Data Types section with the real `ServerSessionAuthenticationTokenRequestData` (5 fields) / `ServerSessionAuthenticationTokenResponseData` (1 field) and the `ServerSessionAuthentication` trait binding. Fixed the unit-test fixture, which used `..Default::default()` on three structs that derive no `Default`. Roster re-derived live: **5** registrations (`authorizedotnet`, `grabpay`, `nuvei`, `paytm`, `payu`), replacing "2 of 77"; the stale 75-name stub roll-call was deleted rather than refreshed. SSAT `get_url` switched from `connector_base_url_payments` to `connector_base_url_merchant_auth`. All numeric `file.rs:NNN` citations re-anchored to symbol names. |
