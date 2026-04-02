# Hipay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/hipay.json
Regenerate: python3 scripts/generators/docs/generate.py hipay
-->

## SDK Configuration

Configure the SDK for hipay:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='hipay')
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
| [PaymentMethodService.Tokenize](#paymentmethodservicetokenize) | Payments | `PaymentMethodServiceTokenizeRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.js) · [Kotlin](../../examples/hipay/kotlin/hipay.kt) · [Rust](../../examples/hipay/rust/hipay.rs)

#### token_authorize

**Examples:** [Python](../../examples/hipay/python/hipay.py) · [JavaScript](../../examples/hipay/javascript/hipay.js) · [Kotlin](../../examples/hipay/kotlin/hipay.kt) · [Rust](../../examples/hipay/rust/hipay.rs)
