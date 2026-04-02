# Cashfree

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/cashfree.json
Regenerate: python3 scripts/generators/docs/generate.py cashfree
-->

## SDK Configuration

Configure the SDK for cashfree:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='cashfree')
client = PaymentClient(config)
```

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
