# Volt

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/volt.json
Regenerate: python3 scripts/generators/docs/generate.py volt
-->

## SDK Configuration

Configure the SDK for volt:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='volt')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/volt/python/volt.py) · [JavaScript](../../examples/volt/javascript/volt.js) · [Kotlin](../../examples/volt/kotlin/volt.kt) · [Rust](../../examples/volt/rust/volt.rs)
