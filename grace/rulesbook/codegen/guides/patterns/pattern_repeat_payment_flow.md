# RepeatPayment Flow Patterns - Grace-UCS Connector Service

## Overview

RepeatPayment flow is used for processing recurring payments using previously stored payment credentials or mandates. This document captures implementation patterns across all connectors in the Grace-UCS connector service.

**Document Version**: 1.0
**Generated**: 2025-11-11
**Total Connectors Analyzed**: 33
**Connectors with Full Implementation**: 7

---

## Table of Contents

1. [Architecture Overview](#architecture-overview)
2. [Connectors with Full Implementation](#connectors-with-full-implementation)
3. [Common Implementation Patterns](#common-implementation-patterns)
4. [Connector-Specific Patterns](#connector-specific-patterns)
5. [Code Examples](#code-examples)
6. [Integration Guidelines](#integration-guidelines)
7. [Best Practices](#best-practices)

---

## Architecture Overview

### Flow Hierarchy

```
RepeatPayment (Flow)
  ├── RepeatPaymentData (Request Type)
  ├── PaymentsResponseData (Response Type)
  └── PaymentFlowData (Resource Common Data)
```

### Core Types

**Flow Type**: `domain_types::connector_flow::RepeatPayment`

**Request Type**: `domain_types::connector_types::RepeatPaymentData`
- Contains mandate_reference for retrieving stored payment credentials
- Includes capture_method for payment capture configuration
- May include split_payments for payment distribution

**Response Type**: `domain_types::connector_types::PaymentsResponseData`
- Standard payment response structure
- Includes transaction status, connector transaction ID, etc.

**Resource Common Data**: `domain_types::connector_types::PaymentFlowData`
- Contains connector configuration, merchant details, etc.

---

## Connectors with Full Implementation

### Summary Table

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse |
|-----------|-------------|--------------|-------------|-------------------|
| **Stripe** | POST | FormUrlEncoded | `v1/payment_intents` | ✅ Same as Authorize |
| **ACI** | POST | FormUrlEncoded | `v1/registrations/{mandate_id}/payments` | ❌ Unique |
| **Cybersource** | POST | JSON | `pts/v2/payments/` | ✅ Same as Authorize |
| **Payload** | POST | TBD | TBD | TBD |
| **Novalnet** | POST | TBD | TBD | TBD |
| **Worldpay** | POST | TBD | TBD | TBD |
| **AuthorizeDotNet** | POST | TBD | TBD | TBD |

### Trait Implementation Status

**Connectors with RepeatPaymentV2 trait only** (26 connectors):
- adyen, bluecode, braintree, cashfree, cashtocode, checkout, cryptopay, dlocal, elavon
- fiserv, fiuu, helcim, mifinity, nexinets, noon, paytm, payu, phonepe, placetopay
- rapyd, razorpay, razorpayv2, trustpay, volt, worldpayvantiv, xendit

These connectors have trait implementations but no macro implementation (empty implementations).

---

## Common Implementation Patterns

### 1. Trait Implementations

All connectors implementing RepeatPayment follow this trait structure:

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2 for ConnectorName<T>
{
}
```

### 2. Macro Configuration

**Pattern**: `create_all_prerequisites!` macro declaration

```rust
macros::create_all_prerequisites!(
    connector_name: ConnectorName,
    generic_type: T,
    api: [
        // ... other flows ...
        (
            flow: RepeatPayment,
            request_body: ConnectorNameRepeatPaymentRequest<T>,
            response_body: ConnectorNameRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>,
        )
    ],
    // ...
);
```

### 3. Flow Implementation

**Pattern**: `macro_connector_implementation!` for the RepeatPayment flow

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: ConnectorName,
    curl_request: FormUrlEncoded(RepeatPaymentRequest) OR Json(RepeatPaymentRequest),
    curl_response: RepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(...) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            // Custom header logic
        }
        fn get_url(...) -> CustomResult<String, errors::IntegrationError> {
            // URL construction logic
        }
    }
);
```

### 4. SourceVerification Implementation

```rust
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData,
        PaymentsResponseData,
    > for ConnectorName<T>
{
}
```

### 5. Mandate Reference Handling

**Common Pattern**: Extract mandate ID from `RepeatPaymentData`

```rust
// Access mandate reference
let mandate_id = req.request.mandate_reference;

// MandateReferenceId enum variants:
// - ConnectorMandateId(connector_mandate_ref)
// - NetworkMandateId(network_mandate_ref)
// - NetworkTokenWithNTI(network_token_with_nti)
```

---

## Connector-Specific Patterns

### Stripe

**File**: `crates/integrations/connector-integration/src/connectors/stripe.rs`

**Key Characteristics**:
- **Request Type Reuse**: Uses `PaymentIntentRequest` (same as Authorize flow)
- **Response Type Reuse**: Uses `PaymentsAuthorizeResponse` (same as Authorize flow)
- **URL**: Same as Authorize flow - `v1/payment_intents`
- **Content Type**: `application/x-www-form-urlencoded`
- **Special Features**: Stripe Connect header handling for split payments

**Type Aliases** (stripe.rs:49):
```rust
PaymentIntentRequest as RepeatPaymentRequest,
PaymentsAuthorizeResponse as RepeatPaymentResponse,
```

**Header Logic** (stripe.rs:433-471):
```rust
fn get_headers(&self, req: &RouterDataV2<RepeatPayment, ...>) {
    let mut header = vec![(CONTENT_TYPE, self.common_get_content_type())];
    let mut api_key = self.get_auth_header(&req.connector_auth_type)?;
    header.append(&mut api_key);

    // Split payment handling for Stripe Connect
    if let Some(transfer_account_id) = /* extract from split_payments */ {
        header.push((STRIPE_COMPATIBLE_CONNECT_ACCOUNT, transfer_account_id));
    }
    Ok(header)
}
```

**URL Pattern** (stripe.rs:474-483):
```rust
fn get_url(&self, req: &RouterDataV2<RepeatPayment, ...>) {
    Ok(format!("{}{}", self.connector_base_url_payments(req), "v1/payment_intents"))
}
```

**Transformer Traits** (stripe/transformers.rs:86-90):
```rust
impl GetRequestIncrementalAuthorization for RepeatPaymentData {
    fn get_request_incremental_authorization(&self) -> Option<bool> {
        None // RepeatPayments don't support incremental authorization
    }
}
```

---

### ACI

**File**: `crates/integrations/connector-integration/src/connectors/aci.rs`

**Key Characteristics**:
- **Request Type**: `AciRepeatPaymentRequest<T>` (unique structure)
- **Response Type**: `AciRepeatPaymentResponse` (alias of `AciPaymentsResponse`)
- **URL**: Unique pattern - `v1/registrations/{mandate_id}/payments`
- **Content Type**: `application/x-www-form-urlencoded`
- **Special Features**: Mandate ID extraction helper function

**Type Alias** (aci.rs:43):
```rust
AciPaymentsResponse as AciRepeatPaymentResponse
```

**Mandate ID Extraction** (aci.rs:304-328):
```rust
pub fn extract_mandate_id(
    &self,
    mandate_reference: &MandateReferenceId,
) -> CustomResult<String, errors::IntegrationError> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
            connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or_else(|| error_stack::report!(
                    errors::IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id"
                    , context: Default::default() }
                ))
        }
        MandateReferenceId::NetworkMandateId(_) => {
            Err(error_stack::report!(errors::IntegrationError::NotImplemented(
                "Network mandate ID not supported for repeat payments in aci".to_string(, Default::default()),
            )))
        }
        MandateReferenceId::NetworkTokenWithNTI(_) => {
            Err(error_stack::report!(errors::IntegrationError::NotImplemented(
                "Network token with NTI not supported for aci".to_string(, Default::default()),
            )))
        }
    }
}
```

**URL Construction** (aci.rs:545-551):
```rust
fn get_url(&self, req: &RouterDataV2<RepeatPayment, ...>) {
    let mandate_id = self.extract_mandate_id(&req.request.mandate_reference)?;
    Ok(format!(
        "{}v1/registrations/{}/payments",
        self.connector_base_url_payments(req),
        mandate_id
    ))
}
```

**Transformer Traits** (aci/transformers.rs:49-53):
```rust
impl GetCaptureMethod for RepeatPaymentData {
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod> {
        self.capture_method
    }
}
```

---

### Cybersource

**File**: `crates/integrations/connector-integration/src/connectors/cybersource.rs`

**Key Characteristics**:
- **Request Type**: `CybersourceRepeatPaymentRequest` (unique structure)
- **Response Type**: `CybersourceRepeatPaymentResponse` (alias of `CybersourcePaymentsResponse`)
- **URL**: Same as Authorize flow - `pts/v2/payments/`
- **Content Type**: `application/json;charset=utf-8`
- **Special Features**: HMAC-SHA256 signature generation, digest computation

**Type Alias** (cybersource.rs:57):
```rust
CybersourcePaymentsResponse as CybersourceRepeatPaymentResponse
```

**Header Generation** (cybersource.rs:869-874):
```rust
fn get_headers(&self, req: &RouterDataV2<RepeatPayment, ...>) {
    self.build_headers(req) // Uses common signature generation logic
}
```

**Signature Generation** (cybersource.rs:250-384):
- Computes SHA-256 digest of request payload
- Generates HMAC-SHA256 signature with merchant credentials
- Includes merchant ID, date, host, and request target in signature string

**URL Pattern** (cybersource.rs:877-884):
```rust
fn get_url(&self, req: &RouterDataV2<RepeatPayment, ...>) {
    Ok(format!("{}pts/v2/payments/", self.connector_base_url_payments(req)))
}
```

---

## Code Examples

### Example 1: Basic RepeatPayment Flow Implementation

```rust
// Step 1: Implement RepeatPaymentV2 trait
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2 for MyConnector<T>
{
}

// Step 2: Add to create_all_prerequisites! macro
macros::create_all_prerequisites!(
    connector_name: MyConnector,
    generic_type: T,
    api: [
        // ... other flows ...
        (
            flow: RepeatPayment,
            request_body: MyConnectorRepeatPaymentRequest<T>,
            response_body: MyConnectorRepeatPaymentResponse,
            router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>,
        )
    ],
    // ...
);

// Step 3: Implement the flow with macro_connector_implementation!
macros::macro_connector_implementation!(
    connector_default_implementations: [get_content_type, get_error_response_v2],
    connector: MyConnector,
    curl_request: Json(MyConnectorRepeatPaymentRequest),
    curl_response: MyConnectorRepeatPaymentResponse,
    flow_name: RepeatPayment,
    resource_common_data: PaymentFlowData,
    flow_request: RepeatPaymentData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>,
        ) -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError> {
            self.build_headers(req)
        }

        fn get_url(
            &self,
            req: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>,
        ) -> CustomResult<String, errors::IntegrationError> {
            Ok(format!("{}api/v1/repeat-payment", self.connector_base_url_payments(req)))
        }
    }
);

// Step 4: Implement SourceVerification
impl<T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize>
    interfaces::verification::SourceVerification<
        RepeatPayment,
        PaymentFlowData,
        RepeatPaymentData,
        PaymentsResponseData,
    > for MyConnector<T>
{
}
```

### Example 2: Transformer Implementation with Mandate Handling

```rust
// In transformers.rs

#[derive(Debug, Serialize)]
pub struct MyConnectorRepeatPaymentRequest<T> {
    pub amount: StringMajorUnit,
    pub currency: String,
    pub mandate_token: String, // Extracted from RepeatPaymentData
    // ... other fields
}

impl<T: PaymentMethodDataTypes> TryFrom<&RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>>
    for MyConnectorRepeatPaymentRequest<T>
{
    type Error = error_stack::Report<errors::IntegrationError>;

    fn try_from(
        item: &RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>
    ) -> Result<Self, Self::Error> {
        // Extract mandate token from request
        let mandate_token = extract_mandate_token(&item.request.mandate_reference)?;

        Ok(Self {
            amount: item.request.amount,
            currency: item.request.currency.to_string(),
            mandate_token,
        })
    }
}

// Helper function to extract mandate token
fn extract_mandate_token(mandate_ref: &MandateReferenceId) -> Result<String, error_stack::Report<errors::IntegrationError>> {
    match mandate_ref {
        MandateReferenceId::ConnectorMandateId(connector_mandate_ref) => {
            connector_mandate_ref
                .get_connector_mandate_id()
                .ok_or_else(|| {
                    error_stack::report!(errors::IntegrationError::MissingRequiredField {
                        field_name: "connector_mandate_id"
                    , context: Default::default() })
                })
        }
        MandateReferenceId::NetworkMandateId(network_mandate_ref) => {
            Ok(network_mandate_ref.network_transaction_id.clone())
        }
        MandateReferenceId::NetworkTokenWithNTI(_) => {
            Err(error_stack::report!(errors::IntegrationError::NotImplemented(
                "Network token with NTI not supported".to_string(, Default::default()),
            )))
        }
    }
}
```

### Example 3: Reusing Authorize Flow Types (Stripe Pattern)

```rust
// In mod.rs
use transformers::{
    PaymentIntentRequest as RepeatPaymentRequest,
    PaymentsAuthorizeResponse as RepeatPaymentResponse,
};

// In create_all_prerequisites! macro
(
    flow: RepeatPayment,
    request_body: RepeatPaymentRequest<T>,  // Reuses PaymentIntentRequest
    response_body: RepeatPaymentResponse,    // Reuses PaymentsAuthorizeResponse
    router_data: RouterDataV2<RepeatPayment, PaymentFlowData, RepeatPaymentData, PaymentsResponseData>,
)
```

---

## Integration Guidelines

### 1. Deciding Between Reuse and Custom Types

**Reuse Authorize Types When:**
- Connector API uses same endpoint for both authorize and repeat payment
- Request/response structure is identical or nearly identical
- Only minor differences (e.g., presence of mandate token vs payment method details)

**Use Custom Types When:**
- Connector has dedicated repeat payment endpoint
- Request structure differs significantly from authorize
- Different authentication or header requirements

### 2. Mandate Reference Handling

**Best Practice**: Create a helper function to extract mandate IDs

```rust
pub fn extract_mandate_id(
    &self,
    mandate_reference: &MandateReferenceId,
) -> CustomResult<String, errors::IntegrationError> {
    match mandate_reference {
        MandateReferenceId::ConnectorMandateId(ref) => {
            ref.get_connector_mandate_id().ok_or(/* error */)
        }
        MandateReferenceId::NetworkMandateId(ref) => {
            // Implement network mandate support if applicable
            Ok(ref.network_transaction_id.clone())
        }
        MandateReferenceId::NetworkTokenWithNTI(_) => {
            Err(/* NotImplemented error */)
        }
    }
}
```

### 3. Split Payment Handling (Stripe Connect Pattern)

If connector supports payment splitting:

```rust
fn get_headers(&self, req: &RouterDataV2<RepeatPayment, ...>) {
    let mut headers = vec![/* base headers */];

    // Extract split payment info
    if let Some(SplitPaymentsRequest::ConnectorSplitPayment(split)) =
        &req.request.split_payments
    {
        if split.charge_type == ChargeType::Direct {
            headers.push((
                "X-Transfer-Account-Id",
                split.transfer_account_id.clone().into_masked(),
            ));
        }
    }

    Ok(headers)
}
```

### 4. Capture Method Handling

Implement trait for capture method retrieval:

```rust
trait GetCaptureMethod {
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod>;
}

impl GetCaptureMethod for RepeatPaymentData {
    fn get_capture_method(&self) -> Option<common_enums::CaptureMethod> {
        self.capture_method
    }
}
```

---

## Best Practices

### 1. Code Organization

**File Structure**:
```
connectors/
├── connector_name.rs              # Main connector implementation
└── connector_name/
    └── transformers.rs            # Request/response transformers
```

**Trait Implementations Order** (in connector_name.rs):
1. Trait declarations (PaymentAuthorizeV2, RepeatPaymentV2, etc.)
2. create_all_prerequisites! macro
3. ConnectorCommon implementation
4. macro_connector_implementation! for each flow
5. Empty trait implementations (stubs)
6. SourceVerification implementations

### 2. Error Handling

**Pattern**: Use descriptive error messages for mandate handling

```rust
MandateReferenceId::NetworkMandateId(_) => {
    Err(error_stack::report!(errors::IntegrationError::NotImplemented(
        format!(
            "Network mandate ID not supported for repeat payments in {}",
            self.id(, Default::default())
        )
    )))
}
```

### 3. Type Safety

**Pattern**: Use type aliases for clarity

```rust
// Instead of repeating complex types
type RepeatPaymentRouterData = RouterDataV2<
    RepeatPayment,
    PaymentFlowData,
    RepeatPaymentData,
    PaymentsResponseData
>;

// Use in function signatures
fn get_url(&self, req: &RepeatPaymentRouterData) -> CustomResult<String, errors::IntegrationError>
```

### 4. Testing Considerations

**Key Test Scenarios**:
1. Repeat payment with connector mandate ID
2. Repeat payment with network mandate ID (if supported)
3. Repeat payment with split payments
4. Error handling for missing mandate reference
5. Error handling for invalid mandate ID format

### 5. Documentation

**Pattern**: Document mandate support clearly

```rust
/// Extracts mandate ID from RepeatPaymentData
///
/// # Supported Mandate Types
/// - ConnectorMandateId: ✅ Supported
/// - NetworkMandateId: ❌ Not Supported
/// - NetworkTokenWithNTI: ❌ Not Supported
///
/// # Returns
/// - Ok(String): Extracted mandate ID
/// - Err: Missing mandate ID or unsupported mandate type
pub fn extract_mandate_id(...) { }
```

---

## Migration Guide

### Converting from Empty Trait to Full Implementation

**Step 1**: Analyze connector API documentation
- Identify repeat payment endpoint
- Understand mandate reference requirements
- Check if dedicated endpoint or reuse authorize endpoint

**Step 2**: Choose implementation strategy
- **Strategy A**: Reuse Authorize types (if API is similar)
- **Strategy B**: Create custom types (if API differs)

**Step 3**: Implement request transformer
```rust
impl<T> TryFrom<&RouterDataV2<RepeatPayment, ...>> for ConnectorRepeatPaymentRequest<T> {
    // Transform RepeatPaymentData to connector format
}
```

**Step 4**: Add to create_all_prerequisites! macro

**Step 5**: Implement macro_connector_implementation!

**Step 6**: Add SourceVerification implementation

**Step 7**: Test with different mandate types

---

## Common Issues and Solutions

### Issue 1: Missing Mandate ID

**Problem**: `MissingRequiredField { field_name: "connector_mandate_id" }`

**Solution**: Ensure mandate ID is properly extracted and validated
```rust
connector_mandate_ref
    .get_connector_mandate_id()
    .ok_or_else(|| report!(errors::IntegrationError::MissingRequiredField {
        field_name: "connector_mandate_id"
    , context: Default::default() }))
```

### Issue 2: Unsupported Mandate Type

**Problem**: Connector doesn't support network mandates

**Solution**: Return explicit `NotImplemented` error
```rust
MandateReferenceId::NetworkMandateId(_) => {
    Err(report!(errors::IntegrationError::NotImplemented(
        "Network mandate not supported".to_string(, Default::default())
    )))
}
```

### Issue 3: URL Construction with Mandate ID

**Problem**: Need to include mandate ID in URL path

**Solution**: Use ACI pattern with mandate extraction
```rust
fn get_url(&self, req: &RepeatPaymentRouterData) {
    let mandate_id = self.extract_mandate_id(&req.request.mandate_reference)?;
    Ok(format!("{}registrations/{}/payments", base_url, mandate_id))
}
```

---

## Appendix

### A. RepeatPaymentData Structure

```rust
pub struct RepeatPaymentData {
    pub amount: MinorUnit,
    pub currency: enums::Currency,
    pub mandate_reference: MandateReferenceId,
    pub capture_method: Option<enums::CaptureMethod>,
    pub split_payments: Option<SplitPaymentsRequest>,
    // ... other fields
}
```

### B. MandateReferenceId Variants

```rust
pub enum MandateReferenceId {
    ConnectorMandateId(ConnectorMandateReferenceId),
    NetworkMandateId(NetworkMandateRef),
    NetworkTokenWithNTI(NetworkTokenWithNTI),
}
```

### C. Related Documentation

- [SetupMandate Flow Patterns] - For initial mandate setup
- [Authorize Flow Patterns] - For standard payment authorization
- [Connector Integration Guide] - General connector implementation guide

---

**Document Maintainers**: Grace-UCS Team
**Last Updated**: 2025-11-11
**Next Review**: When new connectors add RepeatPayment support

## Prerequisite Trait Impls (MANDATORY)

RepeatPayment requires `SetupMandate` to be implemented first. Both flows need their
empty trait marker impls present or the connector will not compile:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::SetupMandateV2<T> for {{ConnectorName}}<T>
{}

impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::RepeatPaymentV2<T> for {{ConnectorName}}<T>
{}
```

No `ValidationTrait` override is needed for RepeatPayment — the flow is dispatched
based on the presence of a stored mandate token in the request, not a `should_do_*` flag.
