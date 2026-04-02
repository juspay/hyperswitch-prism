# Bankofamerica

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/bankofamerica.json
Regenerate: python3 scripts/generators/docs/generate.py bankofamerica
-->

## SDK Configuration

Configure the SDK for bankofamerica:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='bankofamerica')
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
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/bankofamerica/python/bankofamerica.py) · [JavaScript](../../examples/bankofamerica/javascript/bankofamerica.js) · [Kotlin](../../examples/bankofamerica/kotlin/bankofamerica.kt) · [Rust](../../examples/bankofamerica/rust/bankofamerica.rs)
