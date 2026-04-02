# Revolv3

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/revolv3.json
Regenerate: python3 scripts/generators/docs/generate.py revolv3
-->

## SDK Configuration

Configure the SDK for revolv3:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='revolv3')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [proxy_setup_recurring](#proxy_setup_recurring) | Other | `—` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.SetupRecurring](#paymentservicesetuprecurring) | Payments | `PaymentServiceSetupRecurringRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/revolv3/python/revolv3.py) · [JavaScript](../../examples/revolv3/javascript/revolv3.js) · [Kotlin](../../examples/revolv3/kotlin/revolv3.kt) · [Rust](../../examples/revolv3/rust/revolv3.rs)

#### proxy_setup_recurring

**Examples:** [Python](../../examples/revolv3/python/revolv3.py) · [JavaScript](../../examples/revolv3/javascript/revolv3.js) · [Kotlin](../../examples/revolv3/kotlin/revolv3.kt) · [Rust](../../examples/revolv3/rust/revolv3.rs)
