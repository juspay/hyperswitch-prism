# Elavon

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/elavon.json
Regenerate: python3 scripts/generators/docs/generate.py elavon
-->

## SDK Configuration

Configure the SDK for elavon:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='elavon')
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

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/elavon/python/elavon.py) · [JavaScript](../../examples/elavon/javascript/elavon.js) · [Kotlin](../../examples/elavon/kotlin/elavon.kt) · [Rust](../../examples/elavon/rust/elavon.rs)
