# UCS Connector Integration: Comprehensive Step-by-Step Guide

This guide provides a complete, resumable process for integrating payment connectors into the UCS (Universal Connector Service) system. It supports all payment methods and flows, and can be used to continue partial implementations.

> **Important:** This guide is UCS-specific. The architecture differs significantly from traditional Hyperswitch implementations.

## 🏗️ UCS Architecture Overview

### Key Components
```rust
// Core UCS imports for all connectors
use common_utils::{errors::CustomResult, events};
use domain_types::{
    connector_flow::{Authorize, Capture, PSync, RSync, Refund, Void},
    connector_types::{
        // Per-domain flow data - the `ResourceCommonData` slot of RouterDataV2
        PaymentFlowData, RefundFlowData,
        // Flow request/response data
        PaymentVoidData, PaymentsAuthorizeData, PaymentsCaptureData, PaymentsResponseData,
        PaymentsSyncData, RefundSyncData, RefundsData, RefundsResponseData,
        RequestDetails, ResponseId,
    },
    errors::{ConnectorError, IntegrationError, IntegrationErrorContext},
    // Auth arrives here. RouterDataV2 has NO `connector_auth_type` field.
    // `FlowStatus` is what `ErrorResponse::attempt_status` holds - NOT `AttemptStatus`.
    router_data::{ConnectorSpecificConfig, ErrorResponse, FlowStatus},
    router_data_v2::RouterDataV2,
    router_response_types::Response,
};
use interfaces::{
    api::ConnectorCommon,
    connector_integration_v2::ConnectorIntegrationV2,
    // Both NON-GENERIC: one impl per connector, never one per flow.
    decode::BodyDecoding,
    verification::SourceVerification,
};
```

> `RouterDataV2` takes **four** type parameters:
> `RouterDataV2<Flow, ResourceCommonData, FlowSpecificRequest, FlowSpecificResponse>`.
> Omitting `ResourceCommonData` is E0107.

### UCS-Specific Patterns
- **RouterDataV2**: Enhanced type-safe data handling
- **ConnectorIntegrationV2**: Modern trait-based integration
- **Domain Types**: Centralized domain modeling
- **gRPC-first**: All communication via Protocol Buffers
- **Stateless**: No database dependencies

### 🛠️ Utility Functions
UCS provides comprehensive utility functions to avoid code duplication and maintain consistency:
- **Error Handling**: `missing_field_err`, `handle_json_response_deserialization_failure`
- **Amount Conversion**: `convert_amount`, `to_currency_base_unit`, amount convertors
- **Data Transformation**: `to_connector_meta_from_secret`, `convert_uppercase`
- **XML/JSON Processing**: `preprocess_xml_response_bytes`, `serialize_to_xml_string_with_root`
- **Card Processing**: `get_card_details`, `get_card_issuer`
- **Date/Time**: `now`, `get_timestamp_in_milliseconds`, `format_date`

> **📖 Complete Reference:** See [`guides/utility_functions_reference.md`](utility_functions_reference.md) for comprehensive mapping of all utility functions with examples and use cases.

## 🎯 Connector Implementation States

### State Assessment
Before starting, determine your current implementation state:

1. **Fresh Start**: No implementation exists
2. **Partial Core**: Basic auth and authorize flow implemented
3. **Core Complete**: All basic flows working (auth, capture, void, refund)
4. **Extended**: Advanced flows and multiple payment methods
5. **Near Complete**: Only specific flows or payment methods missing
6. **Debug/Fix**: Implementation exists but has issues

## 📋 Complete Flow Coverage

### Core Payment Flows (Priority 1)
- **Authorize**: Initial payment authorization
- **Capture**: Capture authorized amounts
- **Void**: Cancel authorized payments
- **Refund**: Process refunds (full/partial)
- **PSync**: Payment status synchronization
- **RSync**: Refund status synchronization

### Advanced Flows (Priority 2)
- **CreateOrder**: Multi-step payment initiation
- **ServerSessionAuthenticationToken**: Secure session management
- **SetupMandate**: Recurring payment setup
- **RepeatPayment**: Process recurring payments using stored mandates
- **DefendDispute**: Handle chargeback disputes
- **SubmitEvidence**: Submit dispute evidence

### Webhook Integration (Priority 3)
- **IncomingWebhook**: Real-time payment notifications. The trait methods live in
  `crates/types-traits/interfaces/src/connector_types.rs` and return
  `Result<_, error_stack::Report<WebhookError>>` - **not** `IntegrationError`.
  Argument counts are fixed: `get_event_type(&self, RequestDetails)` takes one
  argument besides `&self`; `process_payment_webhook` takes four
  (`RequestDetails`, `Option<ConnectorWebhookSecrets>`,
  `Option<ConnectorSpecificConfig>`, `Option<EventContext>`). Passing three to
  either is E0061.
- **Source verification**: implement the **non-generic** `SourceVerification`
  trait (`interfaces/src/verification.rs`) once for the connector - and
  `BodyDecoding` (`interfaces/src/decode.rs`) the same way.
- **EventMapping**: map the connector's typed event enum to
  `domain_types::connector_types::EventType`. There is no `transformation_status`
  field and no `WebhookTransformationStatus` type - using either is E0560.

## 💳 Payment Method Support

### Card Payments
```rust
PaymentMethodData::Card(card_data) => {
    // Handle all card networks: Visa, Mastercard, Amex, Discover, etc.
    // Handle CVV verification
}
```

## 🛠️ Implementation Process

### Phase 1: Preparation and Planning

#### Step 1.1: Analyze Current State
If resuming partial implementation:
```bash
# AI Command: "analyze current state of [ConnectorName] in UCS"
# The AI will examine existing code and identify:
# - Implemented flows
# - Supported payment methods  
# - Missing functionality
# - Code quality issues
```

#### Step 1.2: Create/Update Technical Specification
```bash
# For new implementation:
# Use: grace-ucs/connector_integration/template/tech_spec.md //change

# For continuing implementation:
# AI will update existing spec with missing components
```

#### Step 1.3: Implementation Planning
```bash
# AI will create detailed plan based on:
# - Current implementation state
# - Missing functionality
# - Priority of remaining work
# Use: grace-ucs/connector_integration/template/planner_steps.md
```

### Phase 2: Core Implementation

#### Step 2.1: Connector Structure Setup
The connector struct itself is **generated** by `macros::create_all_prerequisites!`
(see `crates/integrations/connector-integration/src/connectors/macros.rs:1171`) -
do not hand-write it. You write the trait impls around it.

```rust
// File: crates/integrations/connector-integration/src/connectors/connector_name.rs

macros::create_all_prerequisites!(
    connector_name: ConnectorName,
    generic_type: T,
    api: [
        (
            flow: Authorize,
            request_body: ConnectorNamePaymentsRequest<T>,
            response_body: ConnectorNameAuthorizeResponse,
            router_data: RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>,
        ),
        // ... one entry per implemented flow
    ],
    amount_converters: [
        // Pick the unit type from the vendor spec - see guides/types/types.md
        amount_converter: StringMajorUnit
    ],
    member_functions: {
        // shared helpers
    }
);

impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> ConnectorCommon
    for ConnectorName<T>
{
    fn id(&self) -> &'static str {
        "connector_name"
    }

    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str {
        connectors.connector_name.base_url.as_ref()
    }

    fn get_currency_unit(&self) -> CurrencyUnit {
        // `CurrencyUnit` comes from `common_enums`, not from an `api` module.
        CurrencyUnit::Minor // or Base, depending on connector
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"
    }

    // Real signature: crates/types-traits/interfaces/src/api.rs:50
    // THREE parameters besides &self. The event type is `events::Event`
    // (`common_utils::events`), NOT `ConnectorEvent`.
    fn build_error_response(
        &self,
        res: Response,
        event_builder: Option<&mut events::Event>,
        _connector_config: &ConnectorSpecificConfig,
    ) -> CustomResult<ErrorResponse, ConnectorError> {
        let response: connector_name::ConnectorNameErrorResponse = res
            .response
            .parse_struct("ConnectorNameErrorResponse")
            .change_context(ConnectorError::ResponseDeserializationFailed {
                context: ResponseTransformationErrorContext {
                    http_status_code: Some(res.status_code),
                    additional_context: Some("Failed to parse the error body".to_string()),
                },
            })?;

        // `with_error_response_body!` attaches the body to the event builder.
        // Import it as `use crate::with_error_response_body;`.
        // `set_error_response_body` is NOT a method on `events::Event` - do not call it.
        with_error_response_body!(event_builder, response);

        // Only the connector's own error payload can say whether this is terminal.
        // Anything not known to be terminal stays `None` here.
        let terminal_status = response
            .code
            .as_deref()
            .and_then(connector_name::map_terminal_attempt_status);

        Ok(ErrorResponse {
            status_code: res.status_code,
            // Never `unwrap_or_default()` on a code or message.
            code: response
                .code
                .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
            message: response
                .message
                .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
            reason: response.reason,
            // Flow-aware. `attempt_status` is `Option<FlowStatus>`, so
            // `Some(AttemptStatus::Failure)` is a type error AND the bug that
            // reports a charged payment as FAILURE. A blanket `None` is also
            // wrong - a hard-declined refund then stays Pending and keeps
            // retrying. Derive it from the connector's own error payload and
            // wrap it in the right `FlowStatus` arm (see Pitfall 6 below).
            attempt_status: terminal_status.map(FlowStatus::Payment),
            ..Default::default()
        })
    }
}
```

> The same third-parameter change (`&ConnectorSpecificConfig`) applies to
> `get_error_response_v2` and `get_5xx_error_response` in
> `crates/types-traits/interfaces/src/connector_integration_v2.rs`.
>
> `impl Default for ErrorResponse` exists in
> `crates/types-traits/domain_types/src/router_data.rs`, so prefer
> `..Default::default()` over listing all 13 fields.
>
> **Working exemplar:**
> `crates/integrations/connector-integration/src/connectors/travelhub.rs`.

#### Step 2.2: Authentication Implementation
`RouterDataV2` lost its `connector_auth_type` field on 2026-03-14 (`a7a696c3a`).
Credentials now arrive on `req.connector_config: ConnectorSpecificConfig` as a
**per-connector enum variant**. A `TryFrom<&ConnectorAuthType>` impl no longer
matches the trait and `get_auth_header(&ConnectorAuthType)` is **E0407**.

```rust
// transformers.rs
#[derive(Debug, Clone)]
pub struct ConnectorNameAuthType {
    pub api_key: Secret<String>,
    // Add other auth fields as needed
}

impl TryFrom<&ConnectorSpecificConfig> for ConnectorNameAuthType {
    type Error = error_stack::Report<IntegrationError>;

    fn try_from(auth_type: &ConnectorSpecificConfig) -> Result<Self, Self::Error> {
        match auth_type {
            ConnectorSpecificConfig::ConnectorName { api_key, .. } => Ok(Self {
                api_key: api_key.to_owned(),
            }),
            _ => Err(error_stack::report!(
                IntegrationError::FailedToObtainAuthType {
                    context: IntegrationErrorContext {
                        suggested_action: Some(
                            "Configure the account with ConnectorName credentials".to_string(),
                        ),
                        doc_url: None,
                        additional_context: Some(
                            "ConnectorSpecificConfig variant mismatch".to_string(),
                        ),
                    }
                }
            )),
        }
    }
}
```

```rust
// connector_name.rs - real signature at
// crates/types-traits/interfaces/src/api.rs:25
fn get_auth_header(
    &self,
    auth_type: &ConnectorSpecificConfig,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = connector_name::ConnectorNameAuthType::try_from(auth_type).change_context(
        IntegrationError::FailedToObtainAuthType {
            context: IntegrationErrorContext::default(),
        },
    )?;
    Ok(vec![(
        headers::AUTHORIZATION.to_string(),
        format!("Bearer {}", auth.api_key.peek()).into_masked(),
    )])
}
```

Inside a transformer: `let auth = ConnectorNameAuthType::try_from(&item.connector_config)?;`

#### Step 2.3: Superposition URL Registration & Dynamic URL Patching

> **✅ `add_connector.sh` now does this automatically** as part of scaffolding — it registers the
> connector in `config/superposition.toml` and wires it into `patch_connector_urls` in `types.rs`.
> This section documents what the script produces so you can **verify** it, and how to supply a
> distinct production URL. It became automated after being a manual step (ref PR
> [juspay/hyperswitch-prism#2118](https://github.com/juspay/hyperswitch-prism/pull/2118)).

By default the script writes the single `base_url` to both the sandbox (default) and production
overrides. When the connector has a distinct live URL, pass it explicitly:

```bash
./add_connector.sh {connector_name} {sandbox_base_url} --production-url {production_base_url}
```

**Naming convention** (e.g. `twoc_twop_paco` ↔ `ConnectorEnum::TwocTwopPaco`):
- superposition enum value, `_context_ = { connector = "..." }`, and `patched.<field>` → **snake_case** (`{connector_name}`)
- `ConnectorEnum::<Variant>` → **PascalCase** (`{ConnectorName}`)

**A. `config/superposition.toml`** — the script adds `"{connector_name}"` to the `connector`
dimension `enum` under `[dimensions]` and appends override blocks at the END of the file:

```toml
# {ConnectorName}
[[overrides]]
_context_ = { connector = "{connector_name}" }
connector_base_url = "{sandbox_base_url}"

# {ConnectorName} Production
[[overrides]]
_context_ = { connector = "{connector_name}", environment = "production" }
connector_base_url = "{production_base_url}"
```

**B. `crates/types-traits/domain_types/src/types.rs` → `Connectors::patch_connector_urls()`** — the
script inserts a match arm BEFORE the `_ =>` fallback and adds the name to the fallback's
"Supported connectors:" hint:

```rust
ConnectorEnum::{ConnectorName} => {
    patched.{connector_name}.apply(params_patch);
}
```

> Connectors with extra URL fields (e.g. TrustPay uses `ConnectorParamsWithMoreUrls`) need a
> connector-specific patch struct instead of `params_patch` directly — the script emits the standard
> arm, so adjust it by hand for such cases (mirror the nearest existing arm).

### Phase 3: Flow Implementation

> **📖 Pattern Reference:** For detailed implementation patterns, see:
> - **Authorization Flow**: `guides/patterns/pattern_authorize.md`
> - **Capture Flow**: `guides/patterns/pattern_capture.md`
> - **Refund Flow**: `guides/patterns/pattern_refund.md`
> - **Void Flow**: `guides/patterns/pattern_void.md`
> - **Psync Flow**: `guides/patterns/pattern_psync.md`
> - **Rsync Flow**: `guides/patterns/pattern_rsync.md`
> - **SetupMandate Flow**: `guides/patterns/pattern_setup_mandate.md`
> - **RepeatPayment Flow**: `guides/patterns/pattern_repeat_payment_flow.md`

### Phase 4: Stub Out The Flows You Are Not Implementing

Three macros in
`crates/integrations/connector-integration/src/connectors/macros.rs` generate the
impls a connector needs but does not implement by hand. Read the first matcher arm
of each before using it, and copy a real invocation from a connector file.

**`macro_connector_flow_status_impls!` (~line 1827) - used by 112 of 112 connectors.**
Emits a full `ConnectorIntegrationV2` impl per listed flow whose `get_url` returns
`connector_flow_not_implemented` / `connector_flow_not_supported`. Without it the
connector does not satisfy the dispatcher.

```rust
// crates/integrations/connector-integration/src/connectors/travelhub.rs:455
macros::macro_connector_flow_status_impls!(
    connector: Travelhub,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    not_implemented: [
        Accept, ClientAuthenticationToken, CreateConnectorCustomer, GetConnectorCustomer,
        DefendDispute, MandateRevoke, Authenticate, IncrementalAuthorization, CreateOrder,
        PostAuthenticate, PreAuthenticate, PaymentMethodToken, VoidPC, RepeatPayment,
        ServerAuthenticationToken, ServerSessionAuthenticationToken, SetupMandate,
        SubmitEvidence
    ],
    not_supported: [
        VoidPostRefund,
    ],
);
```

Both list keys are optional; there are entry arms for `not_implemented` alone and
for both together.

**`macro_connector_local_flow_implementation!` (~line 2425)** - for flows with **no
outbound HTTP call** (`build_request_v2` returns `Ok(None)`; everything happens in
`handle_response_v2`).

```rust
// crates/integrations/connector-integration/src/connectors/kount.rs:390
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

**`macro_connector_payout_implementation!` (~line 1448)** - payout flow stubs. Called
with no `payout_flows` list it supplies all nine
(`PayoutCreate, PayoutTransfer, PayoutGet, PayoutVoid, PayoutStage, PayoutCreateLink,
PayoutCreateRecipient, PayoutEnrollDisburseAccount, PayoutEligibility`).

```rust
// crates/integrations/connector-integration/src/connectors/travelhub.rs:187
macros::macro_connector_payout_implementation!(
    connector: Travelhub,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize]
);
```

## 🔄 Resuming Partial Implementation

### Common Resume Scenarios

#### "I have authorize working, need to add capture"
```bash
# AI Command: "add capture flow to existing [ConnectorName] connector in UCS"
# AI will:
# 1. Analyze existing authorize implementation
# 2. Use patterns from guides/patterns/pattern_capture.md
# 3. Create capture flow following same patterns
# 4. Ensure consistency with existing code style
```

## 🚨 Common UCS Pitfalls

### 1. RouterData vs RouterDataV2
```rust
// WRONG (traditional Hyperswitch, and wrong arity)
RouterData<Flow, Request, Response>

// CORRECT (UCS) - FOUR type parameters
RouterDataV2<Flow, ResourceCommonData, FlowSpecificRequest, FlowSpecificResponse>
// e.g. RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
```

`status` is **not** a field on `RouterDataV2`; it lives on
`resource_common_data` (`PaymentFlowData::status`, `RefundFlowData::status`).

### 2. Trait Implementation
```rust
// WRONG (traditional)
ConnectorIntegration<Flow, Request, Response>

// CORRECT (UCS)
ConnectorIntegrationV2<Flow, ResourceCommonData, Request, Response>
```

But `SourceVerification` and `BodyDecoding` are **non-generic**:

```rust
// WRONG - E0107, wrong number of generic arguments
impl<T> SourceVerification<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
    for ConnectorName<T> {}

// CORRECT - ONE impl per connector
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> SourceVerification
    for ConnectorName<T> {}
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize> BodyDecoding
    for ConnectorName<T> {}
```

### 3. Error Handling
```rust
// UCS uses domain_types errors, not hyperswitch_domain_models
use domain_types::errors::{ConnectorError, IntegrationError, WebhookError};
```

`ConnectorError` has **exactly five** variants
(`ResponseDeserializationFailed`, `ResponseHandlingFailed`,
`UnexpectedResponseError`, `IntegrityCheckFailed`, `ConnectorErrorResponse`),
all but the last struct variants requiring a `context`.
`ConnectorError::InvalidData`, `::NotImplemented(..)` and `::InvalidCard` do not
exist (E0599) - those names belong to `IntegrationError`. Read the real
`IntegrationError` variant list in
`crates/types-traits/domain_types/src/errors.rs` before substituting.

### 4. Import Paths
```rust
// UCS-specific imports
use domain_types::*;
use interfaces::connector_integration_v2::*;
// NOT hyperswitch_interfaces or hyperswitch_domain_models
```

### 5. Error code / message defaults
```rust
// WRONG - an empty code and message tell the merchant nothing
code: response.error_code.unwrap_or_default(),
message: response.error_message.unwrap_or_default(),

// CORRECT - crates/common/common_utils/src/consts.rs
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};
code: response.error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
message: response.error_message.unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
```

### 6. Forced terminal status on the shared error path
`ErrorResponse::attempt_status` is `Option<FlowStatus>`, not
`Option<AttemptStatus>`. Hardcoding `Some(AttemptStatus::Failure)` in
`build_error_response` is both a type error and the bug that reports a charged
payment as FAILURE. A blanket `None` is also wrong - a hard-declined refund then
stays Pending and keeps retrying. Be flow-aware:

```rust
// exemplar: connectors/flywire.rs:362-370
let attempt_status: FlowStatus = if is_refund_failure {
    FlowStatus::Refund(RefundStatus::Failure)
} else {
    FlowStatus::Payment(payment_status)
};
// minimal form: connectors/noon.rs:499-512
attempt_status: attempt_status.map(FlowStatus::Payment),
```

### 7. Status mapping needs BOTH halves
```rust
// DESERIALIZATION layer - an unknown wire value must not fail the parse
#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum ConnectorStatus {
    Authorized,
    Captured,
    Failed,
    #[serde(other)]
    Unknown,
}

// STATUS-MAPPING layer - EXHAUSTIVE, no `_ =>` arm, so the compiler flags
// a newly added status instead of silently mapping it to Pending.
let status = match response.status {
    ConnectorStatus::Authorized => AttemptStatus::Authorized,
    ConnectorStatus::Captured   => AttemptStatus::Charged,
    ConnectorStatus::Failed     => AttemptStatus::Failure,
    ConnectorStatus::Unknown    => AttemptStatus::Pending,
};
```

Reviewers require both. A `_ =>` at the mapping layer is rejected; a missing
`#[serde(other)]` at the deserialization layer is also rejected.

### 8. Amount unit selection
There are five unit types in `crates/common/common_utils/src/types.rs`:
`MinorUnit`, `StringMinorUnit`, `StringMajorUnit`, `FloatMajorUnit`,
`StringTwoDecimalUnit`. **Read the vendor spec and match its wire format.**
"Default to `StringMinorUnit` if unclear" is wrong about four times out of five -
the real HEAD distribution is StringMajorUnit 34, FloatMajorUnit 26, MinorUnit 21,
StringMinorUnit 19.

### 9. In-band 2xx failures
A 200 response can still carry a declined payment. Branch on a success predicate
and return `Err(ErrorResponse { .. })` from the response transformer so the
failure is surfaced rather than mapped to a success status. Reference:
`utils::is_payment_failure` in `crates/types-traits/domain_types/src/utils.rs`.

## 📊 Implementation Checklist

### Core Implementation ✅
- [ ] Connector structure via `macros::create_all_prerequisites!`
- [ ] Auth via `TryFrom<&ConnectorSpecificConfig>` (NOT `&ConnectorAuthType`)
- [ ] Authorize flow
- [ ] Capture flow
- [ ] Void flow
- [ ] Refund flow
- [ ] Payment sync
- [ ] Refund sync
- [ ] Error handling
- [ ] `macro_connector_flow_status_impls!` covering every unimplemented flow
- [ ] `macro_connector_payout_implementation!` (payout stubs)
- [ ] One non-generic `SourceVerification` impl and one `BodyDecoding` impl

### Payment Methods ✅
- [ ] Card payments (all networks)

### Contract Compliance ✅
- [ ] `RouterDataV2` used with all four type parameters
- [ ] `build_error_response` takes `(res, Option<&mut events::Event>, &ConnectorSpecificConfig)`
- [ ] `ErrorResponse` literals compile against the real 13 fields (or use `..Default::default()`)
- [ ] `attempt_status` is `Option<FlowStatus>` and flow-aware, never a hardcoded terminal value
- [ ] `TransactionResponse` lists all 11 fields; `RefundsResponseData` all 4
- [ ] Error code/message fall back to `NO_ERROR_CODE` / `NO_ERROR_MESSAGE`
- [ ] Status enum has `#[serde(other)] Unknown`; status mapping has no `_ =>` arm
- [ ] Amount unit type matches the vendor's documented wire format
- [ ] In-band 2xx failure returns `Err(ErrorResponse { .. })`
- [ ] Webhook methods use `WebhookError` and the correct argument counts
- [ ] No `transformation_status` / `WebhookTransformationStatus` anywhere

### Quality & Testing ✅
- [ ] cargo build works for all flows

## 🎯 Success Metrics

A complete UCS connector implementation should:
1. **Support all relevant payment methods** for the connector
2. **Handle all core flows** (auth, capture, void, refund, sync)
3. **Follow UCS patterns** consistently
4. **Handle errors gracefully** with proper mapping
5. **Should be error free** should build successfully without any compilation errors

Remember: GRACE makes connector development resumable at any stage. You can always continue where you left off!