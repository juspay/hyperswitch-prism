# Bambora

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/bambora.json
Regenerate: python3 scripts/generators/docs/generate.py bambora
-->

## SDK Configuration

Configure the SDK for bambora:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='bambora')
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

**Examples:** [Python](../../examples/bambora/python/bambora.py) · [JavaScript](../../examples/bambora/javascript/bambora.js) · [Kotlin](../../examples/bambora/kotlin/bambora.kt) · [Rust](../../examples/bambora/rust/bambora.rs)
