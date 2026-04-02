# Powertranz

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/powertranz.json
Regenerate: python3 scripts/generators/docs/generate.py powertranz
-->

## SDK Configuration

Configure the SDK for powertranz:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='powertranz')
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

**Examples:** [Python](../../examples/powertranz/python/powertranz.py) · [JavaScript](../../examples/powertranz/javascript/powertranz.js) · [Kotlin](../../examples/powertranz/kotlin/powertranz.kt) · [Rust](../../examples/powertranz/rust/powertranz.rs)
