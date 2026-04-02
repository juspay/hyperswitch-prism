# CyberSource

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/cybersource.json
Regenerate: python3 scripts/generators/docs/generate.py cybersource
-->

## SDK Configuration

Configure the SDK for cybersource:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='cybersource')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentMethodAuthenticationService.PreAuthenticate](#paymentmethodauthenticationservicepreauthenticate) | Authentication | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Authentication

#### PaymentMethodAuthenticationService.PreAuthenticate

Initiate 3DS flow before payment authorization. Collects device data and prepares authentication context for frictionless or challenge-based verification.

| | Message |
|---|---------|
| **Request** | `PaymentMethodAuthenticationServicePreAuthenticateRequest` |
| **Response** | `PaymentMethodAuthenticationServicePreAuthenticateResponse` |

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.js#L351) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt) · [Rust](../../examples/cybersource/rust/cybersource.rs)

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.js) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt) · [Rust](../../examples/cybersource/rust/cybersource.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/cybersource/python/cybersource.py) · [JavaScript](../../examples/cybersource/javascript/cybersource.js) · [Kotlin](../../examples/cybersource/kotlin/cybersource.kt) · [Rust](../../examples/cybersource/rust/cybersource.rs)
