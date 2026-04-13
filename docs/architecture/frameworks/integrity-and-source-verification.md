# Integrity and Source Verification

Every payload that you receive from a payment processor carries two inherent risks: 
(i) Data tampering risk
(ii) Impersonation risk

Prism provides strong **run-time checks** to eliminate both risks. This section also includes some recommended best practices to developers using the library.

| Verification Type | What It Checks | Attack Prevented |
|-------------------|----------------|------------------|
| **Integrity** | Amount, currency, and transaction ID match your records | Data tampering |
| **Source** | Cryptographic signature using shared secrets | Impersonation, forged webhooks |

### Data Tampering (Integrity Risk)

An attacker intercepting a webhook can modify the payload before it reaches your server. The attacker will be able to exploit by:
- Changing a failed payment status to "succeeded", which might deceive you to ship unpaid orders
- Modifying the amount from $100 to $1, effectively making the customer pays less than expected.

### Impersonation (Source Risk)

It is possible for attackers can forge webhooks that appear to come from payment processors. This can be exploited by:
- Sending fake "payment succeeded" webhooks, which might deceive you to ship unpaid orders
- Mimicking refund notifications, to manipulate your accounting systems

Prism provides built-in verification for both risks, and it is strongly recommended to enable them and test them before using on production.

## How Prism helps with Integrity and Source Verification?

### Request and Response Comparison

Prism uses a `FlowIntegrity` trait to compare request and response data in a strongly typed fashion. The core implementation is available in [`backend/interfaces/src/integrity.rs`](../../backend/interfaces/src/integrity.rs):

```rust
/// Trait for integrity objects that can perform field-by-field comparison
pub trait FlowIntegrity {
    /// The integrity object type for this flow
    type IntegrityObject;

    /// Compare request and response integrity objects
    fn compare(
        req_integrity_object: Self::IntegrityObject,
        res_integrity_object: Self::IntegrityObject,
        connector_transaction_id: Option<String>,
    ) -> Result<(), IntegrityCheckError>;
}

/// Trait for data types that can provide integrity objects
pub trait GetIntegrityObject<T: FlowIntegrity> {
    /// Extract integrity object from response data
    fn get_response_integrity_object(&self) -> Option<T::IntegrityObject>;

    /// Generate integrity object from request data
    fn get_request_integrity_object(&self) -> T::IntegrityObject;
}
```

### Amount and Currency Verification

During the payment authorization step, Prism extracts amount and currency from the request and compares with the response during run-time. The core implementation is available in [`backend/interfaces/src/integrity.rs`](../../backend/interfaces/src/integrity.rs):

```rust
// From backend/interfaces/src/integrity.rs
impl<T: PaymentMethodDataTypes> GetIntegrityObject<AuthoriseIntegrityObject>
    for PaymentsAuthorizeData<T>
{
    fn get_response_integrity_object(&self) -> Option<AuthoriseIntegrityObject> {
        self.integrity_object.clone()
    }

    fn get_request_integrity_object(&self) -> AuthoriseIntegrityObject {
        AuthoriseIntegrityObject {
            amount: self.minor_amount,
            currency: self.currency,
        }
    }
}
```

Webhooks payloads also include amounts (might vary across payment processors). Prism verifies these match your records.

```json
{
  "event_type": "PAYMENT_INTENT_SUCCESS",
  "amount": {
    "minor_amount": 5999,
    "currency": "USD"
  },
  "expected_amount": {
    "minor_amount": 5999,
    "currency": "USD"
  }
}
```

If amounts mismatch, Prism flags the discrepancy clearly. In such cases you should reject the webhook and use direct server-to-server APIs to cross validate the information.

```json
{
  "error": {
    "code": "AMOUNT_MISMATCH",
    "message": "Webhook amount does not match expected amount",
    "webhook_amount": 4999,
    "expected_amount": 5999,
    "currency": "USD"
  }
}
```

## Signature Verification

Typically Payment processors sign webhooks with a shared secret, to verify the payload against pre-configured secrets. An Authorize.net might use a SHA512, whereas a PPRO might use a SHA256. 

Prism handles the signature verification across multiple processors.

And below are real examples from one of the connectors on how it is implemented [`backend/connector-integration/src/connectors/authorizedotnet.rs`](../../backend/connector-integration/src/connectors/authorizedotnet.rs):

```rust
fn verify_webhook_source(
    &self,
    request: RequestDetails,
    connector_webhook_secret: Option<ConnectorWebhookSecrets>,
) -> Result<bool, error_stack::Report<WebhookError>> {
    let webhook_secret = match connector_webhook_secret {
        Some(secrets) => secrets.secret,
        None => return Ok(false),
    };

    // Extract X-ANET-Signature header (case-insensitive)
    let signature_header = request
        .headers
        .get("X-ANET-Signature")
        .or_else(|| request.headers.get("x-anet-signature"))?;

    // Parse "sha512=<hex>" format
    let signature_hex = match signature_header.strip_prefix("sha512=") {
        Some(hex) => hex,
        None => return Ok(false),
    };

    // Decode hex signature
    let expected_signature = match hex::decode(signature_hex) {
        Ok(sig) => sig,
        Err(_) => return Ok(false),
    };

    // Compute HMAC-SHA512 of request body
    use common_utils::crypto::{HmacSha512, SignMessage};
    let crypto_algorithm = HmacSha512;
    let computed_signature = crypto_algorithm
        .sign_message(&webhook_secret, &request.body)?;

    // Constant-time comparison to prevent timing attacks
    Ok(computed_signature == expected_signature)
}
```

If verification fails, Prism flags the discrepancy very clearly.

```json
{
  "error": {
    "code": "SIGNATURE_VERIFICATION_FAILED",
    "message": "Webhook signature does not match payload",
    "connector": "stripe",
    "suggestion": "Check your webhook secret is correct and the payload was not modified"
  }
}
```

## Recommendations for Developers

### Verify the Transaction ID and Amount in your system

It is strongly recommended to track transaction IDs across the lifecycle. When a webhook or API response arrives, check the following parameters in your application database, before updating them.

| Check | If check fails? |
|-------|--------------|
| ID exists in system | Reject unknown transaction IDs |
| Status transition valid | Reject invalid state changes (e.g., SUCCEEDED → PENDING) |
| Amount match | Reject payload to prevent amount tampering |
| Currency match | Reject payload to prevent currency tampering |

### Always Configure the secrets while enabling a processor

Some payment processors may have the secrets as optional configuration/ implementation. Always, generate new secret in processor dashboard and update the Prism configuration accordingly. 

An example configuration for Stripe as below.

```javascript
const { PaymentClient } = require('hyperswitch-prism');

// Webhook secrets are configured per-connector in connectorConfig
// Note: Webhook verification is done at the application level
// by comparing signatures using the webhook secret
const config = {
    connectorConfig: {
        stripe: {
            apiKey: { value: process.env.STRIPE_API_KEY }
        }
    }
};
const paymentClient = new PaymentClient(config);
```

## Next Steps

- [Error Handling](../architecture/error-handling.md) — Handle verification failures in your application
