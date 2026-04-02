# Barclaycard

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/barclaycard.json
Regenerate: python3 scripts/generators/docs/generate.py barclaycard
-->

## SDK Configuration

Configure the SDK for barclaycard:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='barclaycard')
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

**Examples:** [Python](../../examples/barclaycard/python/barclaycard.py) · [JavaScript](../../examples/barclaycard/javascript/barclaycard.js) · [Kotlin](../../examples/barclaycard/kotlin/barclaycard.kt) · [Rust](../../examples/barclaycard/rust/barclaycard.rs)
