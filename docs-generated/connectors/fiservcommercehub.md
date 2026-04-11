# Fiservcommercehub

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/fiservcommercehub.json
Regenerate: python3 scripts/generators/docs/generate.py fiservcommercehub
-->

## SDK Configuration

Use this config for all flows in this connector. Replace `YOUR_API_KEY` with your actual credentials.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Set credentials before running (field names depend on connector auth type):
# config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
#     fiservcommercehub=payment_pb2.FiservcommercehubConfig(api_key=...),
# ))

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { ConnectorClient } = require('connector-service-node-ffi');

// Reuse this client for all flows
const client = new ConnectorClient({
    connector: 'Fiservcommercehub',
    environment: 'sandbox',
    connector_auth_type: {
        header_key: { api_key: 'YOUR_API_KEY' },
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setConnector("Fiservcommercehub")
    .setEnvironment(Environment.SANDBOX)
    .setAuth(
        ConnectorAuthType.newBuilder()
            .setHeaderKey(HeaderKey.newBuilder().setApiKey("YOUR_API_KEY"))
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use connector_service_sdk::{ConnectorClient, ConnectorConfig};

let config = ConnectorConfig {
    connector: "Fiservcommercehub".to_string(),
    environment: Environment::Sandbox,
    auth: ConnectorAuth::HeaderKey { api_key: "YOUR_API_KEY".into() },
    ..Default::default()
};
```

</details>

</td>
</tr>
</table>

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [MerchantAuthenticationService.CreateServerAuthenticationToken](#merchantauthenticationservicecreateserverauthenticationtoken) | Authentication | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| [FraudService.Get](#fraudserviceget) | Other | `FraudServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L125) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L113) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L96) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L114)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L143) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L125) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L128)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L134) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L122) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L106) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L121)

### Authentication

#### MerchantAuthenticationService.CreateServerAuthenticationToken

Generate short-lived connector authentication token. Provides secure credentials for connector API access without storing secrets client-side.

| | Message |
|---|---------|
| **Request** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenRequest` |
| **Response** | `MerchantAuthenticationServiceCreateServerAuthenticationTokenResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L107) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L95) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L78) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L100)

### Other

#### FraudService.Get

Retrieves fraud decision history and risk scores for a specific transaction. Supports customer service investigations and chargeback dispute preparation.

| | Message |
|---|---------|
| **Request** | `FraudServiceGetRequest` |
| **Response** | `FraudServiceGetResponse` |

**Examples:** [Python](../../examples/fiservcommercehub/fiservcommercehub.py#L116) · [TypeScript](../../examples/fiservcommercehub/fiservcommercehub.ts#L104) · [Kotlin](../../examples/fiservcommercehub/fiservcommercehub.kt#L88) · [Rust](../../examples/fiservcommercehub/fiservcommercehub.rs#L107)
