# Worldpayvantiv

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/worldpayvantiv.json
Regenerate: python3 scripts/generators/docs/generate.py worldpayvantiv
-->

## SDK Configuration

Configure the SDK for worldpayvantiv:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='worldpayvantiv')
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
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/worldpayvantiv/python/worldpayvantiv.py) · [JavaScript](../../examples/worldpayvantiv/javascript/worldpayvantiv.js) · [Kotlin](../../examples/worldpayvantiv/kotlin/worldpayvantiv.kt) · [Rust](../../examples/worldpayvantiv/rust/worldpayvantiv.rs)
