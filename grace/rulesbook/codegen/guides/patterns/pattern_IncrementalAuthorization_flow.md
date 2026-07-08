# Incremental Authorization Pattern - FINAL UNIFIED DOCUMENT
## RL Loop Iterations 1-9 Consolidated

---

## Executive Summary

This document provides a **unified pattern** for implementing **Incremental Authorization** across multiple payment connectors (Stripe, PayPal, CyberSource) based on analysis of 3 production implementations.

---

## 1. Flow Definition

```rust
use domain_types::connector_flow::IncrementalAuthorization;

// Request/Response Types
use domain_types::connector_types::{
    PaymentsIncrementalAuthorizationData,
    PaymentsResponseData,
};
```

---

## 2. Request Data Structure

### Input
```rust
pub struct PaymentsIncrementalAuthorizationData {
    /// Amount to increment (in minor units - cents for USD)
    pub minor_amount: MinorUnit,

    /// Currency code (ISO 4217)
    pub currency: Currency,

    /// Optional reason for increment
    pub reason: Option<String>,

    /// Original payment's connector transaction ID
    pub connector_transaction_id: ResponseId,

    /// Additional connector-specific metadata
    pub connector_metadata: Option<SecretSerdeValue>,
}
```

### Request Body Variants

| Connector | Format | Structure | Example ($20.00) |
|-----------|--------|-----------|------------------|
| **Stripe** | Form URL Encoded | `amount: MinorUnit` | `amount=2000` |
| **PayPal** | JSON | `amount: { currency_code, value }` | `{"amount":{"currency_code":"USD","value":"20.00"}}` |
| **CyberSource** | JSON | `orderInformation.amountDetails` | `{"orderInformation":{"amountDetails":{"totalAmount":"20.00","currency":"USD"}}}` |

---

## 3. URL Construction

### Base URL Pattern
```rust
fn connector_base_url_payments<'a>(
    &self,
    req: &'a RouterDataV2<...>
) -> &'a str {
    &req.resource_common_data.connectors.{connector}.base_url
}
```

### Endpoint Paths

| Connector | Endpoint Pattern |
|-----------|-----------------|
| Stripe | `/v1/payment_intents/{id}/increment_authorization` |
| PayPal | `/v2/payments/authorizations/{id}/reauthorize` |
| CyberSource | `/pts/v2/payments/{id}/incrementalAuthorizations` |

### URL Construction Code
```rust
fn get_url(
    &self,
    req: &RouterDataV2<IncrementalAuthorization, PaymentFlowData,
                       PaymentsIncrementalAuthorizationData, PaymentsResponseData>,
) -> CustomResult<String, IntegrationError> {
    let original_payment_id = req.request.connector_transaction_id
        .get_connector_transaction_id()
        .change_context(IntegrationError::MissingConnectorTransactionID)?;

    Ok(format!(
        "{}{}",
        self.connector_base_url_payments(req),
        self.build_incremental_auth_path(&original_payment_id)
    ))
}
```

---

## 4. Authentication Patterns

### Stripe: Bearer Token
```rust
fn get_auth_header(&self, auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = stripe::StripeAuthType::try_from(auth_type)?;
    Ok(vec![
        ("Authorization".to_string(),
         format!("Bearer {}", auth.api_key.peek()).into_masked()),
        ("Stripe-Version".to_string(), "2023-10-16".to_string().into_masked()),
    ])
}
```

### PayPal: OAuth 2.0
```rust
fn get_auth_header(&self, auth_type: &ConnectorAuthType) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let auth = paypal::PaypalAuthType::try_from(auth_type)?;
    let credentials = format!("{}:{}", auth.api_key.peek(), auth.key1.peek());
    let encoded = BASE64_ENGINE.encode(credentials);

    // Step 1: Get access token
    let token = self.get_oauth_token(&encoded)?;

    Ok(vec![
        ("Authorization".to_string(),
         format!("Bearer {}", token).into_masked()),
        ("PayPal-Request-Id".to_string(),
         generate_uuid().into_masked()),
    ])
}
```

### CyberSource: HMAC-SHA256
```rust
fn generate_cybersource_headers(
    &self,
    auth: &CybersourceAuthType,
    payload: &str,
    endpoint: &str,
    method: &str,
) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
    let date = OffsetDateTime::now_utc().format(&Rfc2822)?;
    let host = "api.cybersource.com";
    let digest = sha256_hash(payload);

    let signature_string = format!(
        "host: {}\ndate: {}\n(request-target): {} {}\ndigest: {}\nv-c-merchant-id: {}",
        host, date, method.to_lowercase(), endpoint, digest, auth.api_key.peek()
    );

    let signature = hmac_sha256(auth.key1.peek(), &signature_string);
    let auth_header = format!(
        r#"Signature keyid="{}", algorithm="HmacSHA256", headers="host date (request-target) digest v-c-merchant-id", signature="{}""#,
        auth.api_key.peek(), signature
    );

    Ok(vec![
        ("host".to_string(), host.into_masked()),
        ("date".to_string(), date.into_masked()),
        ("v-c-merchant-id".to_string(), auth.api_key.peek().to_string().into_masked()),
        ("Authorization".to_string(), auth_header.into_masked()),
        ("digest".to_string(), digest.into_masked()),
    ])
}
```

---

## 5. Response Handling

### Response Structure
```rust
#[derive(Debug, Deserialize)]
pub struct ConnectorIncrementalAuthResponse {
    pub id: String,
    pub status: ConnectorPaymentStatus,
    pub amount: AmountType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}
```

### Status Mapping

| Connector Status | Internal AttemptStatus |
|-----------------|------------------------|
| `requires_capture` / `CREATED` / `AUTHORIZED` | `Authorized` |
| `processing` / `PENDING` | `Pending` |
| (error) / `DENIED` / `DECLINED` | `Failure` |
| `canceled` / `VOIDED` | `Voided` |

### Response Handler
```rust
fn handle_response(
    &self,
    data: &RouterDataV2<...>,
    event_builder: Option<&mut ConnectorEvent>,
    res: Response,
) -> CustomResult<RouterDataV2<...>, ConnectorError> {
    let response: ConnectorIncrementalAuthResponse = res
        .response
        .parse_struct("IncrementalAuthResponse")
        .change_context(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

    event_builder.map(|event| event.set_response_body(&response));

    let status = match response.status {
        ConnectorPaymentStatus::Authorized => AttemptStatus::Authorized,
        ConnectorPaymentStatus::Pending => AttemptStatus::Pending,
        ConnectorPaymentStatus::Declined => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,
    };

    RouterDataV2 {
        response: Ok(PaymentsResponseData {
            status: Some(status),
            connector_transaction_id: Some(response.id),
        }),
        ..data.clone()
    }
}
```

---

## 6. Error Handling

```rust
fn get_error_response(
    &self,
    res: Response,
    event_builder: Option<&mut ConnectorEvent>,
) -> CustomResult<ErrorResponse, ConnectorError> {
    let response = res
        .response
        .parse_struct("ErrorResponse")
        .change_context(ConnectorError::ResponseDeserializationFailed { context: Default::default() })?;

    event_builder.map(|event| event.set_error_response_body(&response));

    Ok(ErrorResponse {
        status_code: res.status_code,
        code: response.code,
        message: response.message,
        reason: response.reason,
        attempt_status: None,
        connector_transaction_id: None,
    })
}
```

### Common Error Codes
- `amount_too_large` / `AMOUNT_TOO_LARGE` / `EXCEEDS_AUTHORIZATION_AMOUNT`
- `payment_intent_unexpected_state`
- `AUTHORIZATION_ALREADY_CAPTURED`
- `AUTHORIZATION_EXPIRED`

---

## 7. Preconditions & Validation

### Required Preconditions
1. Original payment must be in **AUTHORIZED** state
2. Increment must occur **BEFORE** capture
3. Total authorized amount cannot exceed **115%** of original

### Validation Code
```rust
fn validate_incremental_auth_request(
    data: &PaymentsIncrementalAuthorizationData,
    original_payment: &PaymentAttempt,
) -> CustomResult<(), IntegrationError> {
    // Check amount is positive
    if data.minor_amount <= MinorUnit::zero() {
        return Err(IntegrationError::InvalidRequestBody)?;
    }

    // Check connector_transaction_id exists
    if data.connector_transaction_id.is_none() {
        return Err(IntegrationError::MissingConnectorTransactionID)?;
    }

    // Check payment state
    if original_payment.status != AttemptStatus::Authorized {
        return Err(IntegrationError::PaymentNotAuthorized)?;
    }

    // Check amount limit (115% rule)
    const MAX_PERCENTAGE: f64 = 1.15;
    let max_allowed = MinorUnit::from(
        (original_payment.amount.inner() as f64 * MAX_PERCENTAGE) as i64
    );
    let new_total = original_payment.authorized_amount + data.minor_amount;

    if new_total > max_allowed {
        return Err(IntegrationError::AmountTooLarge)?;
    }

    Ok(())
}
```

---

## 8. Complete Macro Implementation

```rust
macros::macro_connector_implementation!(
    connector_default_implementations: [
        get_content_type,
        get_error_response_v2
    ],
    connector: {ConnectorName},
    curl_request: {Json|FormUrlEncoded}(IncrementalAuthRequest),
    curl_response: IncrementalAuthResponse,
    flow_name: IncrementalAuthorization,
    resource_common_data: PaymentFlowData,
    flow_request: PaymentsIncrementalAuthorizationData,
    flow_response: PaymentsResponseData,
    http_method: Post,
    generic_type: T,
    [PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize],
    other_functions: {
        fn get_headers(&self, req: &RouterDataV2<...>) -> CustomResult<Vec<(String, Maskable<String>)>, IntegrationError> {
            self.build_auth_headers(req)
        }

        fn get_url(&self, req: &RouterDataV2<...>) -> CustomResult<String, IntegrationError> {
            let payment_id = req.request.connector_transaction_id
                .get_connector_transaction_id()
                .change_context(IntegrationError::MissingConnectorTransactionID)?;
            Ok(format!(
                "{}v1/payment_intents/{}/increment_authorization",
                self.connector_base_url_payments(req),
                payment_id
            ))
        }

        fn get_request_body(
            &self,
            req: &RouterDataV2<...>,
        ) -> CustomResult<RequestContent, IntegrationError> {
            let request = IncrementalAuthRequest::try_from(&req.request)?;
            Ok(RequestContent::FormUrlEncoded(request))
        }
    }
);
```

---

## 9. Connector-Specific Checklist

### Stripe
- [ ] Use `FormUrlEncoded` request format
- [ ] Endpoint: `/v1/payment_intents/{id}/increment_authorization`
- [ ] Auth: Bearer token
- [ ] Amount: Pass MinorUnit directly

### PayPal
- [ ] Use `Json` request format
- [ ] Endpoint: `/v2/payments/authorizations/{id}/reauthorize`
- [ ] Auth: OAuth Bearer (Basic → Bearer exchange)
- [ ] Amount: Convert to `StringMajorUnit`
- [ ] Add `PayPal-Request-Id` header

### CyberSource
- [ ] Use `Json` request format
- [ ] Endpoint: `/pts/v2/payments/{id}/incrementalAuthorizations`
- [ ] Auth: HMAC-SHA256 signature
- [ ] Amount: Convert to `StringMajorUnit`
- [ ] Include `host`, `date`, `digest`, `v-c-merchant-id` headers

---

## 10. Testing Checklist

- [ ] Test with valid incremental amount
- [ ] Test with amount exceeding 115% limit
- [ ] Test on already captured payment
- [ ] Test on voided payment
- [ ] Test with invalid payment ID
- [ ] Verify status mapping
- [ ] Verify error handling

---

## References

- Stripe: https://docs.stripe.com/api/payment_intents/increment_authorization
- PayPal: https://developer.paypal.com/docs/api/payments/v2/#authorizations_reauthorize
- CyberSource: https://developer.cybersource.com/docs/cybs/en-us/payments/developer/all/rest/payments/incremental-authorization.html

---

**Document Version**: 9.0 (Final Consolidated)
**Generated**: RL Loop Iterations 1-9
**Connectors Analyzed**: Stripe, PayPal, CyberSource
**Pattern Confidence**: High (based on 3 production implementations)
