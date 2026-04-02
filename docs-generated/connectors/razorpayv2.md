# Razorpay V2

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/razorpayv2.json
Regenerate: python3 scripts/generators/docs/generate.py razorpayv2
-->

## SDK Configuration

Configure the SDK for razorpayv2:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='razorpayv2')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [token_authorize](#token_authorize) | Other | `—` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs)

#### token_authorize

**Examples:** [Python](../../examples/razorpayv2/python/razorpayv2.py) · [JavaScript](../../examples/razorpayv2/javascript/razorpayv2.js) · [Kotlin](../../examples/razorpayv2/kotlin/razorpayv2.kt) · [Rust](../../examples/razorpayv2/rust/razorpayv2.rs)
