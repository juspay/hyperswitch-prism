# Celero

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/celero.json
Regenerate: python3 scripts/generators/docs/generate.py celero
-->

## SDK Configuration

Configure the SDK for celero:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='celero')
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

**Examples:** [Python](../../examples/celero/python/celero.py) · [JavaScript](../../examples/celero/javascript/celero.js) · [Kotlin](../../examples/celero/kotlin/celero.kt) · [Rust](../../examples/celero/rust/celero.rs)
