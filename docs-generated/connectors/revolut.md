# Revolut

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/revolut.json
Regenerate: python3 scripts/generators/docs/generate.py revolut
-->

## SDK Configuration

Configure the SDK for revolut:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='revolut')
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
| [token_authorize](#token_authorize) | Other | `—` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/revolut/python/revolut.py) · [JavaScript](../../examples/revolut/javascript/revolut.js) · [Kotlin](../../examples/revolut/kotlin/revolut.kt) · [Rust](../../examples/revolut/rust/revolut.rs)

#### token_authorize

**Examples:** [Python](../../examples/revolut/python/revolut.py) · [JavaScript](../../examples/revolut/javascript/revolut.js) · [Kotlin](../../examples/revolut/kotlin/revolut.kt) · [Rust](../../examples/revolut/rust/revolut.rs)
