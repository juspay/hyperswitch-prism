# Paytm

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/paytm.json
Regenerate: python3 scripts/generators/docs/generate.py paytm
-->

## SDK Configuration

Configure the SDK for paytm:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='paytm')
client = PaymentClient(config)
```

## Integration Scenarios

Complete, runnable examples for common integration patterns. Each example shows the full flow with status handling. Copy-paste into your app and replace placeholder values.

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Authorize](#paymentserviceauthorize) | Payments | `PaymentServiceAuthorizeRequest` |
| [MerchantAuthenticationService.CreateSessionToken](#merchantauthenticationservicecreatesessiontoken) | Authentication | `MerchantAuthenticationServiceCreateSessionTokenRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |

### Authentication

#### MerchantAuthenticationService.CreateSessionToken

Create session token for payment processing. Maintains session state across multiple payment operations for improved security and tracking.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateSessionTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateSessionTokenResponse` |

**Examples:** [Python](../../examples/paytm/python/paytm.py) · [JavaScript](../../examples/paytm/javascript/paytm.js) · [Kotlin](../../examples/paytm/kotlin/paytm.kt) · [Rust](../../examples/paytm/rust/paytm.rs)
