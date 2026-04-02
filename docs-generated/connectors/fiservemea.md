# Fiservemea

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiservemea.json
Regenerate: python3 scripts/generators/docs/generate.py fiservemea
-->

## SDK Configuration

Configure the SDK for fiservemea:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='fiservemea')
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

**Examples:** [Python](../../examples/fiservemea/python/fiservemea.py) · [JavaScript](../../examples/fiservemea/javascript/fiservemea.js) · [Kotlin](../../examples/fiservemea/kotlin/fiservemea.kt) · [Rust](../../examples/fiservemea/rust/fiservemea.rs)
