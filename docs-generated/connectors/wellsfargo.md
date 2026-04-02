# Wellsfargo

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/wellsfargo.json
Regenerate: python3 scripts/generators/docs/generate.py wellsfargo
-->

## SDK Configuration

Configure the SDK for wellsfargo:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='wellsfargo')
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

**Examples:** [Python](../../examples/wellsfargo/python/wellsfargo.py) · [JavaScript](../../examples/wellsfargo/javascript/wellsfargo.js) · [Kotlin](../../examples/wellsfargo/kotlin/wellsfargo.kt) · [Rust](../../examples/wellsfargo/rust/wellsfargo.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/wellsfargo/python/wellsfargo.py) · [JavaScript](../../examples/wellsfargo/javascript/wellsfargo.js) · [Kotlin](../../examples/wellsfargo/kotlin/wellsfargo.kt) · [Rust](../../examples/wellsfargo/rust/wellsfargo.rs)
