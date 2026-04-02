# Getnet

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/getnet.json
Regenerate: python3 scripts/generators/docs/generate.py getnet
-->

## SDK Configuration

Configure the SDK for getnet:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='getnet')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.js#L258) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs)

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/getnet/python/getnet.py) · [JavaScript](../../examples/getnet/javascript/getnet.js) · [Kotlin](../../examples/getnet/kotlin/getnet.kt) · [Rust](../../examples/getnet/rust/getnet.rs)
