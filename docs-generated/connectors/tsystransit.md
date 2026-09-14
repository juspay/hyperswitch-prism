# Tsystransit

<!--
This file is auto-generated. Do not edit by hand.
Source: data/field_probe/tsystransit.json
Regenerate: python3 scripts/generators/docs/generate.py tsystransit
-->

## SDK Configuration

Use this config for all flows in this connector. Replace the placeholders `YOUR_DEVICE_ID`, `YOUR_TRANSACTION_KEY`, `YOUR_DEVELOPER_ID`, `YOUR_MERCHANT_STREET_ADDRESS`, `YOUR_CUSTOMER_SERVICE_PHONE_NUMBER`, `YOUR_MERCHANT_URL`, `YOUR_BASE_URL` with your actual values.

<table>
<tr><td><b>Python</b></td><td><b>JavaScript</b></td><td><b>Kotlin</b></td><td><b>Rust</b></td></tr>
<tr>
<td valign="top">

<details><summary>Python</summary>

```python
from payments.generated import sdk_config_pb2, payment_pb2, events_pb2, payment_methods_pb2

config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
    connector_config=payment_pb2.ConnectorSpecificConfig(
        tsys_transit=payment_pb2.TsysTransitConfig(
            device_id=payment_methods_pb2.SecretString(value="YOUR_DEVICE_ID"),
            transaction_key=payment_methods_pb2.SecretString(value="YOUR_TRANSACTION_KEY"),
            developer_id=payment_methods_pb2.SecretString(value="YOUR_DEVELOPER_ID"),
            merchant_street_address=payment_methods_pb2.SecretString(value="YOUR_MERCHANT_STREET_ADDRESS"),
            customer_service_phone_number=payment_methods_pb2.SecretString(value="YOUR_CUSTOMER_SERVICE_PHONE_NUMBER"),
            merchant_url="YOUR_MERCHANT_URL",
            base_url="YOUR_BASE_URL",
        ),
    ),
)

```

</details>

</td>
<td valign="top">

<details><summary>JavaScript</summary>

```javascript
const { PaymentClient } = require('hyperswitch-prism');
const { ConnectorConfig, Environment, Connector } = require('hyperswitch-prism').types;

const config = ConnectorConfig.create({
    connector: Connector.TSYS_TRANSIT,
    environment: Environment.SANDBOX,
    auth: {
        tsysTransit: {
            deviceId: { value: 'YOUR_DEVICE_ID' },
            transactionKey: { value: 'YOUR_TRANSACTION_KEY' },
            developerId: { value: 'YOUR_DEVELOPER_ID' },
            merchantStreetAddress: { value: 'YOUR_MERCHANT_STREET_ADDRESS' },
            customerServicePhoneNumber: { value: 'YOUR_CUSTOMER_SERVICE_PHONE_NUMBER' },
            merchantUrl: 'YOUR_MERCHANT_URL',
            baseUrl: 'YOUR_BASE_URL',
        }
    },
});
```

</details>

</td>
<td valign="top">

<details><summary>Kotlin</summary>

```kotlin
val config = ConnectorConfig.newBuilder()
    .setOptions(SdkOptions.newBuilder().setEnvironment(Environment.SANDBOX).build())
    .setConnectorConfig(
        ConnectorSpecificConfig.newBuilder()
            .setTsysTransit(TsysTransitConfig.newBuilder()
                .setDeviceId(SecretString.newBuilder().setValue("YOUR_DEVICE_ID").build())
                .setTransactionKey(SecretString.newBuilder().setValue("YOUR_TRANSACTION_KEY").build())
                .setDeveloperId(SecretString.newBuilder().setValue("YOUR_DEVELOPER_ID").build())
                .setMerchantStreetAddress(SecretString.newBuilder().setValue("YOUR_MERCHANT_STREET_ADDRESS").build())
                .setCustomerServicePhoneNumber(SecretString.newBuilder().setValue("YOUR_CUSTOMER_SERVICE_PHONE_NUMBER").build())
                .setMerchantUrl("YOUR_MERCHANT_URL")
                .setBaseUrl("YOUR_BASE_URL")
                .build())
            .build()
    )
    .build()
```

</details>

</td>
<td valign="top">

<details><summary>Rust</summary>

```rust
use grpc_api_types::payments::*;
use grpc_api_types::payments::connector_specific_config;

let config = ConnectorConfig {
    connector_config: Some(ConnectorSpecificConfig {
            config: Some(connector_specific_config::Config::TsysTransit(TsysTransitConfig {
                device_id: Some(hyperswitch_masking::Secret::new("YOUR_DEVICE_ID".to_string())),  // Authentication credential
                transaction_key: Some(hyperswitch_masking::Secret::new("YOUR_TRANSACTION_KEY".to_string())),  // Authentication credential
                developer_id: Some(hyperswitch_masking::Secret::new("YOUR_DEVELOPER_ID".to_string())),  // Authentication credential
                merchant_street_address: Some(hyperswitch_masking::Secret::new("YOUR_MERCHANT_STREET_ADDRESS".to_string())),  // Authentication credential
                customer_service_phone_number: Some(hyperswitch_masking::Secret::new("YOUR_CUSTOMER_SERVICE_PHONE_NUMBER".to_string())),  // Authentication credential
                merchant_url: Some("YOUR_MERCHANT_URL".to_string()),  // Endpoint URL, e.g. https://sandbox.example.com
                base_url: Some("YOUR_BASE_URL".to_string()),  // Endpoint URL, e.g. https://sandbox.example.com
                ..Default::default()
            })),
        }),
    options: Some(SdkOptions {
        environment: Environment::Sandbox.into(),
    }),
};
```

</details>

</td>
</tr>
</table>

## API Reference

| Flow (Service.RPC) | Category | gRPC Request Message |
|--------------------|----------|----------------------|
| [PaymentService.Capture](#paymentservicecapture) | Payments | `PaymentServiceCaptureRequest` |
| [PaymentService.Get](#paymentserviceget) | Payments | `PaymentServiceGetRequest` |
| [PaymentService.Refund](#paymentservicerefund) | Payments | `PaymentServiceRefundRequest` |
| [RefundService.Get](#refundserviceget) | Refunds | `RefundServiceGetRequest` |
| [PaymentService.Reverse](#paymentservicereverse) | Payments | `PaymentServiceReverseRequest` |
| [PaymentService.Void](#paymentservicevoid) | Payments | `PaymentServiceVoidRequest` |

### Payments

#### PaymentService.Capture

Finalize an authorized payment by transferring funds. Captures the authorized amount to complete the transaction and move funds to your merchant account.

| | Message |
|---|---------|
| **Request** | `PaymentServiceCaptureRequest` |
| **Response** | `PaymentServiceCaptureResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.ts#L90) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.kt#L94) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.rs)

#### PaymentService.Get

Retrieve current payment status from the payment processor. Enables synchronization between your system and payment processors for accurate state tracking.

| | Message |
|---|---------|
| **Request** | `PaymentServiceGetRequest` |
| **Response** | `PaymentServiceGetResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.ts#L99) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.kt#L104) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.rs)

#### PaymentService.Refund

Process a partial or full refund for a captured payment. Returns funds to the customer when goods are returned or services are cancelled.

| | Message |
|---|---------|
| **Request** | `PaymentServiceRefundRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.ts#L108) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.kt#L112) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.rs)

#### PaymentService.Reverse

Reverse a captured payment in full. Initiates a complete refund when you need to cancel a settled transaction rather than just an authorization.

| | Message |
|---|---------|
| **Request** | `PaymentServiceReverseRequest` |
| **Response** | `PaymentServiceReverseResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.ts#L126) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.kt#L134) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.rs)

#### PaymentService.Void

Cancel an authorized payment that has not been captured. Releases held funds back to the customer's payment method when a transaction cannot be completed.

| | Message |
|---|---------|
| **Request** | `PaymentServiceVoidRequest` |
| **Response** | `PaymentServiceVoidResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.ts) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.kt#L142) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.rs)

### Refunds

#### RefundService.Get

Retrieve refund status from the payment processor. Tracks refund progress through processor settlement for accurate customer communication.

| | Message |
|---|---------|
| **Request** | `RefundServiceGetRequest` |
| **Response** | `RefundResponse` |

**Examples:** [Python](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.py) · [TypeScript](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.ts#L117) · [Kotlin](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.kt#L122) · [Rust](https://github.com/juspay/hyperswitch-prism/blob/main/examples/tsystransit/tsystransit.rs)
