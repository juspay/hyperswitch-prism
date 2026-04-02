# Forte

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/forte.json
Regenerate: python3 scripts/generators/docs/generate.py forte
-->

## SDK Configuration

Configure the SDK for forte:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='forte')
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
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/forte/python/forte.py) · [JavaScript](../../examples/forte/javascript/forte.js) · [Kotlin](../../examples/forte/kotlin/forte.kt) · [Rust](../../examples/forte/rust/forte.rs)
