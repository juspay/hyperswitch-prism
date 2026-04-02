# Novalnet

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/novalnet.json
Regenerate: python3 scripts/generators/docs/generate.py novalnet
-->

## SDK Configuration

Configure the SDK for novalnet:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='novalnet')
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
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [RecurringPaymentService.Charge](#recurringpaymentservicecharge) | Mandates | `RecurringPaymentServiceChargeRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.js) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt) · [Rust](../../examples/novalnet/rust/novalnet.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/novalnet/python/novalnet.py) · [JavaScript](../../examples/novalnet/javascript/novalnet.js) · [Kotlin](../../examples/novalnet/kotlin/novalnet.kt) · [Rust](../../examples/novalnet/rust/novalnet.rs)
