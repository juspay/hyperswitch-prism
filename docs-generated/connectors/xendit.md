# Xendit

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/xendit.json
Regenerate: python3 scripts/generators/docs/generate.py xendit
-->

## SDK Configuration

Configure the SDK for xendit:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='xendit')
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

**Examples:** [Python](../../examples/xendit/python/xendit.py) · [JavaScript](../../examples/xendit/javascript/xendit.js) · [Kotlin](../../examples/xendit/kotlin/xendit.kt) · [Rust](../../examples/xendit/rust/xendit.rs)
