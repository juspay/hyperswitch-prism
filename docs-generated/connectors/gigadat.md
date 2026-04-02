# Gigadat

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/gigadat.json
Regenerate: python3 scripts/generators/docs/generate.py gigadat
-->

## SDK Configuration

Configure the SDK for gigadat:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='gigadat')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
