# Datatrans

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/datatrans.json
Regenerate: python3 scripts/generators/docs/generate.py datatrans
-->

## SDK Configuration

Configure the SDK for datatrans:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='datatrans')
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

**Examples:** [Python](../../examples/datatrans/python/datatrans.py) · [JavaScript](../../examples/datatrans/javascript/datatrans.js) · [Kotlin](../../examples/datatrans/kotlin/datatrans.kt) · [Rust](../../examples/datatrans/rust/datatrans.rs)
