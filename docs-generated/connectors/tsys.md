# Tsys

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/tsys.json
Regenerate: python3 scripts/generators/docs/generate.py tsys
-->

## SDK Configuration

Configure the SDK for tsys:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='tsys')
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
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/tsys/python/tsys.py) · [JavaScript](../../examples/tsys/javascript/tsys.js) · [Kotlin](../../examples/tsys/kotlin/tsys.kt) · [Rust](../../examples/tsys/rust/tsys.rs)
