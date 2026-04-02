# TrustPay

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/trustpay.json
Regenerate: python3 scripts/generators/docs/generate.py trustpay
-->

## SDK Configuration

Configure the SDK for trustpay:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='trustpay')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| [PaymentService.CreateOrder](#paymentservicecreateorder) | Payments | `PaymentServiceCreateOrderRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [proxy_authorize](#proxy_authorize) | Other | `—` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.js#L177) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs)

### Other

#### proxy_authorize

**Examples:** [Python](../../examples/trustpay/python/trustpay.py) · [JavaScript](../../examples/trustpay/javascript/trustpay.js) · [Kotlin](../../examples/trustpay/kotlin/trustpay.kt) · [Rust](../../examples/trustpay/rust/trustpay.rs)
