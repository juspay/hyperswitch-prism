# Silverflow

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/silverflow.json
Regenerate: python3 scripts/generators/docs/generate.py silverflow
-->

## SDK Configuration

Configure the SDK for silverflow:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='silverflow')
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

**Examples:** [Python](../../examples/silverflow/python/silverflow.py) · [JavaScript](../../examples/silverflow/javascript/silverflow.js) · [Kotlin](../../examples/silverflow/kotlin/silverflow.kt) · [Rust](../../examples/silverflow/rust/silverflow.rs)
