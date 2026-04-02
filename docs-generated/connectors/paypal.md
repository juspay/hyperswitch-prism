# Paypal

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paypal.json
Regenerate: python3 scripts/generators/docs/generate.py paypal
-->

## SDK Configuration

Configure the SDK for paypal:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='paypal')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/paypal/python/paypal.py) · [JavaScript](../../examples/paypal/javascript/paypal.js#L328) · [Kotlin](../../examples/paypal/kotlin/paypal.kt) · [Rust](../../examples/paypal/rust/paypal.rs)

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/paypal/python/paypal.py) · [JavaScript](../../examples/paypal/javascript/paypal.js) · [Kotlin](../../examples/paypal/kotlin/paypal.kt) · [Rust](../../examples/paypal/rust/paypal.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/paypal/python/paypal.py) · [JavaScript](../../examples/paypal/javascript/paypal.js) · [Kotlin](../../examples/paypal/kotlin/paypal.kt) · [Rust](../../examples/paypal/rust/paypal.rs)
