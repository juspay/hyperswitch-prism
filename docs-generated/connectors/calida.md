# Calida

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/calida.json
Regenerate: python3 scripts/generators/docs/generate.py calida
-->

## SDK Configuration

Configure the SDK for calida:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='calida')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
