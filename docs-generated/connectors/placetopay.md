# PlacetoPay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/placetopay.json
Regenerate: python3 scripts/generators/docs/generate.py placetopay
-->

## SDK Configuration

Configure the SDK for placetopay:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='placetopay')
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
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |
