# Itaubank

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/itaubank.json
Regenerate: python3 scripts/generators/docs/generate.py itaubank
-->

## SDK Configuration

Configure the SDK for itaubank:

```python
from payments import PaymentClient, ConnectorConfig, Environment

config = ConnectorConfig(connector='itaubank')
client = PaymentClient(config)
```

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [MerchantAuthenticationService.CreateAccessToken](#merchantauthenticationservicecreateaccesstoken) | Authentication | `MerchantAuthenticationServiceCreateAccessTokenRequest` |

### Authentication

#### MerchantAuthenticationService.CreateAccessToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateAccessTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateAccessTokenResponse` |

**Examples:** [Python](../../examples/itaubank/python/itaubank.py) · [JavaScript](../../examples/itaubank/javascript/itaubank.js) · [Kotlin](../../examples/itaubank/kotlin/itaubank.kt) · [Rust](../../examples/itaubank/rust/itaubank.rs)
