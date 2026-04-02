# Trustpayments

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/trustpayments.json
Regenerate: python3 scripts/generators/docs/generate.py trustpayments
-->

## SDK Configuration

Configure the SDK for trustpayments:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='trustpayments')
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

**Examples:** [Python](../../examples/trustpayments/python/trustpayments.py) · [JavaScript](../../examples/trustpayments/javascript/trustpayments.js) · [Kotlin](../../examples/trustpayments/kotlin/trustpayments.kt) · [Rust](../../examples/trustpayments/rust/trustpayments.rs)
