# Hyperpg

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/hyperpg.json
Regenerate: python3 scripts/generators/docs/generate.py hyperpg
-->

## SDK Configuration

Configure the SDK for hyperpg:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='hyperpg')
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

**Examples:** [Python](../../examples/hyperpg/python/hyperpg.py) · [JavaScript](../../examples/hyperpg/javascript/hyperpg.js) · [Kotlin](../../examples/hyperpg/kotlin/hyperpg.kt) · [Rust](../../examples/hyperpg/rust/hyperpg.rs)
