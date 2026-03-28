# Card Payment (Authorize + Capture)

Reserve funds with `authorize`, then settle with a separate `capture` call. Use for physical goods or delayed fulfillment.

## Overview

```
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  Authorize  │────→│   Capture   │────→│   Settled   │
│  (Hold $)   │     │  (Take $)   │     │  (Complete) │
└─────────────┘     └─────────────┘     └─────────────┘
      │
      └────→ Void (if cancelled before capture)
```

## Quick Start

### Python

```python
from payments import PaymentClient
from payments.generated import sdk_config_pb2, payment_pb2

# 1. Configure (connector-agnostic pattern)
config = sdk_config_pb2.ConnectorConfig(
    connector="stripe",  # Change to any supported connector
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
# Set credentials based on connector auth type
config.connector_config.CopyFrom(payment_pb2.ConnectorSpecificConfig(
    stripe=payment_pb2.StripeConfig(api_key="sk_test_..."),
))

client = PaymentClient(config)

# 2. Authorize (reserve funds)
auth_response = client.authorize(
    merchant_transaction_id="txn_001",
    amount={"minor_amount": 1000, "currency": "USD"},
    payment_method={
        "card": {
            "card_number": {"value": "4111111111111111"},
            "card_exp_month": {"value": "12"},
            "card_exp_year": {"value": "2025"},
            "card_cvc": {"value": "123"},
        }
    },
    capture_method="MANUAL",  # Key: MANUAL = authorize+capture separate
)

# 3. Handle response status
if auth_response.status == "AUTHORIZED":
    connector_txn_id = auth_response.connector_transaction_id
    # Proceed to capture
elif auth_response.status == "PENDING":
    # Wait for webhook, then poll get()
    pass
elif auth_response.status == "FAILED":
    # Show error to customer
    pass

# 4. Capture (settle funds)
capture_response = client.capture(
    merchant_capture_id="capture_001",
    connector_transaction_id=connector_txn_id,
    amount_to_capture={"minor_amount": 1000, "currency": "USD"},
)
```

### Rust

```rust
use grpc_api_types::payments::*;
use hyperswitch_payments_client::ConnectorClient;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Configure
    let config = ConnectorConfig {
        connector: "stripe".to_string(),
        environment: Environment::Sandbox.into(),
        auth: ConnectorAuth::HeaderKey { api_key: "sk_test_...".into() },
        ..Default::default()
    };
    let client = ConnectorClient::new(config, None)?;

    // 2. Authorize
    let auth_req = PaymentServiceAuthorizeRequest {
        merchant_transaction_id: "txn_001".into(),
        amount: Some(Amount { minor_amount: 1000, currency: "USD".into() }),
        payment_method: Some(PaymentMethod {
            payment_method: Some(payment_method::PaymentMethod::Card(Card {
                card_number: "4111111111111111".into(),
                card_exp_month: "12".into(),
                card_exp_year: "2025".into(),
                card_cvc: "123".into(),
                ..Default::default()
            })),
        }),
        capture_method: "MANUAL".into(),
        ..Default::default()
    };

    let auth_res = client.authorize(auth_req).await?;

    // 3. Handle status
    match auth_res.status() {
        PaymentStatus::Authorized => {
            let txn_id = auth_res.connector_transaction_id;
            // Capture
            let capture_req = PaymentServiceCaptureRequest {
                merchant_capture_id: "capture_001".into(),
                connector_transaction_id: txn_id,
                amount_to_capture: Some(Amount { minor_amount: 1000, currency: "USD".into() }),
                ..Default::default()
            };
            let _ = client.capture(capture_req).await?;
        }
        PaymentStatus::Pending => println!("Awaiting async confirmation..."),
        PaymentStatus::Failure => println!("Payment declined"),
        _ => {}
    }

    Ok(())
}
```

## Connector Support

| Connector | Card Support | 3DS Support | Auth Window | Notes |
|-----------|--------------|-------------|-------------|-------|
| **Stripe** | ✅ All cards | ✅ Full | 7 days | Best for US/EU |
| **Adyen** | ✅ All cards | ✅ Full | 28 days | Best for EU/UK |
| **Checkout.com** | ✅ All cards | ✅ Full | 7 days | Strong APAC |
| **Authorize.net** | ✅ Visa/MC | ✅ Partial | 30 days | US only |
| **Worldpay** | ✅ All cards | ✅ Full | 14 days | Enterprise focus |
| **+ 68 more** | [View all →](../connectors/index.md#card-payments) | | | |

## Status Handling Reference

| Status | Meaning | Next Action |
|--------|---------|-------------|
| `AUTHORIZED` | Funds reserved successfully | Proceed to capture |
| `PENDING` | Async processing (3DS, bank auth) | Poll `get()` or await webhook |
| `FAILED` | Declined or error | Surface to customer, don't retry |
| `CANCELLED` | Voided before capture | Start new payment if needed |

## Connector-Specific Variations

<details>
<summary><b>Stripe</b> — Default, recommended</summary>

No special handling needed. Standard flow works as documented above.

```python
# Stripe-specific: use confirm=true for immediate processing
auth_response = client.authorize(..., confirm=True)
```
</details>

<details>
<summary><b>Adyen</b> — EU/UK optimized</summary>

Adyen requires `shopper_reference` for recurring:

```python
auth_response = client.authorize(
    ...,
    customer={"merchant_customer_id": "cust_001"},  # Required for EU
    shopper_reference="cust_001",  # Adyen-specific
)
```
</details>

<details>
<summary><b>Checkout.com</b> — APAC optimized</summary>

Checkout.com uses `3ds.enabled` instead of `auth_type`:

```python
auth_response = client.authorize(
    ...,
    three_ds={"enabled": True},  # Instead of auth_type="THREE_DS"
)
```
</details>

## Common Issues

### "capture_method not supported"

Some connectors only support `AUTOMATIC` (single-step). Use [Auto-Capture scenario](./checkout-autocapture.md) instead.

### Auth expires before capture

Capture timing varies by connector (see table above). Set reminders to capture within the auth window.

## Related Scenarios

- **[Auto-Capture](./checkout-autocapture.md)** — Single-step authorize+capture (digital goods)
- **[Refunds](./refund.md)** — Return funds after capture
- **[3D Secure](./authentication.md)** — Add authentication for compliance
- **[Recurring](./recurring.md)** — Set up subscriptions

## Full Examples

Runnable examples for this scenario:

```bash
# Python
python examples/scenarios/checkout_card.py --connector=stripe

# Rust
cargo run --example checkout_card -- --connector=stripe
```

See [examples/scenarios/](../../examples/scenarios/) for all connectors and languages.
