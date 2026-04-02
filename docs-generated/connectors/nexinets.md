# Nexinets

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/nexinets.json
Regenerate: python3 scripts/generators/docs/generate.py nexinets
-->

## SDK Configuration

Configure the SDK for nexinets:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='nexinets')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/nexinets/python/nexinets.py) · [JavaScript](../../examples/nexinets/javascript/nexinets.js) · [Kotlin](../../examples/nexinets/kotlin/nexinets.kt) · [Rust](../../examples/nexinets/rust/nexinets.rs)
