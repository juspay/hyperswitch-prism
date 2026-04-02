# Loonio

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/loonio.json
Regenerate: python3 scripts/generators/docs/generate.py loonio
-->

## SDK Configuration

Configure the SDK for loonio:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='loonio')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
