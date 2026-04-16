# TrustPayments Apple Pay — Testing Reference

PR: #1046 | UCS Branch: `feat/grace-trustpayments-applepay`

---

## 1. How the Flow Works (Architecture)

TrustPayments Apple Pay uses the **decrypted-passthrough** pattern:

```
Client (encrypted token)
    → HS Router (decrypts token using PPC cert)
        → UCS (receives decrypted DPAN fields)
            → TrustPayments (AUTH with walletsource=APPLEPAY)
```

### Key Components

**HS Router** handles decryption. Decryption is triggered by MCA `metadata` containing `apple_pay_combined/simplified`. Without this metadata, the router does NOT decrypt — it tries to tokenize instead, which breaks TrustPayments (no tokenize URL in UCS).

**`apple_pay_pre_decrypt_flow = "network_tokenization"`** in `development.toml` tells the router: once the token IS decrypted (i.e., `PaymentMethodToken::ApplePayDecrypt`), skip the Tokenize step and go straight to Authorize. This only activates *after* decryption has already happened.

**`trustpayments` is in `ucs_only_connectors`** — all TrustPayments traffic automatically routes through UCS. No extra config needed for that.

---

## 2. Credentials

| Field | Value |
|-------|-------|
| Site Reference | `test_juspay140213` |
| Webservices User ID | `ws@justpay.in` |
| Webservices Password | `H!*!tm7C,ksm` |
| MCA ID | `mca_InejAqff2e9mX4aPO7kx` |
| Merchant ID | `merchant_finix_test_1` |
| Profile ID | `pro_U10S78u93RI2U3812Yqo` |
| Merchant API Key | `dev_Zr5cL5uWlqB0OykxliYrptTHw7tSb11i810e6GGnAfSruxlyL9JIqCVEqFWhpzk0` |
| Admin API Key | `test_admin` |

### gRPC Auth Headers

```
-H "x-connector: trustpayments"
-H "x-auth: signature-key"
-H "x-merchant-id: merchant_finix_test_1"
-H 'x-api-key: ws@justpay.in'
-H 'x-key1: H!*!tm7C,ksm'
-H 'x-api-secret: test_juspay140213'
```

> **Note:** Use **single quotes** for headers containing `!` to prevent zsh history expansion.

---

## 3. Local Environment

| Service | Port | Log File |
|---------|------|----------|
| HS Router | 8080 | `/tmp/hs-router.log` |
| UCS | 8000 | `/tmp/ucs-trustpayments.log` |
| UCS Metrics | 8085 | — |
| SDK | 9050 | — |
| Demo App | 5252 | — |

DB: `hyperswitch_db` — connect via `psql hyperswitch_db`

**Always flush Redis after any DB or config change:**
```bash
redis-cli FLUSHALL
```

---

## 4. Required Setup — HS Router (`config/development.toml`)

All four of these must be present. They are already set in the local environment.

### A. pm_filters — apple_pay entry
```toml
[pm_filters.trustpayments]
google_pay = { country = "US,GB,AU,CA,...", currency = "USD,EUR,GBP,..." }
apple_pay = { country = "US,GB,AU,CA,...", currency = "USD,EUR,GBP,..." }
```

### B. tokenization — pre_decrypt_flow
```toml
trustpayments = {
  long_lived_token = false,
  payment_method = "wallet",
  apple_pay_pre_decrypt_flow = "network_tokenization",
  google_pay_pre_decrypt_flow = "network_tokenization"
}
```

### C. zero_mandates & mandates — apple_pay connector_list
```toml
wallet.apple_pay.connector_list = "...,trustpayments"
```
(trustpayments must be in both `[zero_mandates]` and `[mandates]`)

### D. Apple Pay cert config
```toml
[applepay_merchant_configs]
# ... merchant cert for Apple Pay session
[applepay_decrypt_keys]
apple_pay_ppc = "MIIEiT..."       # Payment Processing Certificate (DER base64)
apple_pay_ppc_key = "-----BEGIN EC PRIVATE KEY-----\n..."
```

PPC cert's `publicKeyHash` (SHA-256 of uncompressed public key):
`IXgIwh5Oo2LtW5/lbld2SeQMyR483ofgeNEu+j4emeo=`

Merchant ID in PPC cert: `merchant.com.stripe.sang`

---

## 5. Required Setup — Database

### 5a. UCS rollout configs (already inserted)
```sql
INSERT INTO configs (key, config) VALUES
  ('ucs_rollout_config_merchant_finix_test_1_trustpayments_wallet_apple_pay_Authorize',
   '{"rollout_percent": 1.0, "http_url": "http://localhost:8000", "https_url": "http://localhost:8000", "execution_mode": "primary"}'),
  ('ucs_rollout_config_merchant_finix_test_1_trustpayments_wallet_apple_pay_PaymentMethodToken',
   '{"rollout_percent": 1.0, "http_url": "http://localhost:8000", "https_url": "http://localhost:8000", "execution_mode": "primary"}')
ON CONFLICT (key) DO UPDATE SET config = EXCLUDED.config;
```

### 5b. MCA payment_methods_enabled — must include apple_pay (already set)
```sql
UPDATE merchant_connector_account
SET payment_methods_enabled = ARRAY['{
  "payment_method":"wallet",
  "payment_method_types":[
    {"payment_method_type":"google_pay",...},
    {"payment_method_type":"apple_pay","payment_experience":"invoke_sdk_client","card_networks":null,
     "accepted_currencies":null,"accepted_countries":null,"minimum_amount":1,"maximum_amount":68607706,
     "recurring_enabled":false,"installment_payment_enabled":false}
  ]
}']::json[]
WHERE merchant_connector_id = 'mca_InejAqff2e9mX4aPO7kx';
```

### 5c. MCA metadata — CRITICAL: triggers decryption at HS Router (already set)

This is the single most important config. Without it, the router sends the encrypted token
to UCS's Tokenize step instead of decrypting it first, causing `relative URL without a base`.

```sql
UPDATE merchant_connector_account
SET metadata = '{
  "apple_pay_combined": {
    "simplified": {
      "payment_request_data": {
        "supported_networks": ["visa", "masterCard", "amex"],
        "merchant_capabilities": ["supports3DS"],
        "label": "TrustPayments"
      },
      "session_token_data": {
        "initiative_context": "hyperswitch.io"
      }
    }
  }
}'::jsonb
WHERE id = 'mca_InejAqff2e9mX4aPO7kx';
```

**Verify it's set:**
```bash
psql hyperswitch_db -c "SELECT metadata FROM merchant_connector_account WHERE id = 'mca_InejAqff2e9mX4aPO7kx';"
```
Should return a non-null `apple_pay_combined` JSON object.

### 5d. Active routing — TrustPayments (already set)

Routing `routing_LhRnb4IdVrM0EZeIeElg` is permanently active for Apple Pay PR testing.

```bash
psql hyperswitch_db -t -c "SELECT routing_algorithm FROM business_profile WHERE profile_id = 'pro_U10S78u93RI2U3812Yqo';"
# Should show: {"algorithm_id":"routing_LhRnb4IdVrM0EZeIeElg",...}
```

To switch routing if needed:
```bash
psql hyperswitch_db -c "UPDATE business_profile
  SET routing_algorithm = '{\"algorithm_id\":\"routing_LhRnb4IdVrM0EZeIeElg\",\"timestamp\":1776334295,\"config_algo_id\":null,\"surcharge_config_algo_id\":null}'
  WHERE profile_id = 'pro_U10S78u93RI2U3812Yqo';"
redis-cli FLUSHALL
```

---

## 6. Full E2E Test — HS Router via cURL

This tests the complete flow: HS Router decrypts token → UCS → TrustPayments → CHARGED.

The `payment_data` value is a **base64-encoded EC_v1 Apple Pay token** (the full JSON blob).

```bash
curl -s -X POST http://localhost:8080/payments \
  -H "Content-Type: application/json" \
  -H "api-key: dev_Zr5cL5uWlqB0OykxliYrptTHw7tSb11i810e6GGnAfSruxlyL9JIqCVEqFWhpzk0" \
  -d '{
    "amount": 1000,
    "currency": "GBP",
    "profile_id": "pro_U10S78u93RI2U3812Yqo",
    "confirm": true,
    "capture_method": "automatic",
    "payment_method": "wallet",
    "payment_method_type": "apple_pay",
    "payment_method_data": {
      "wallet": {
        "apple_pay": {
          "payment_data": "<BASE64_ENCODED_APPLE_PAY_TOKEN>",
          "payment_method": {
            "display_name": "Visa 4242",
            "network": "Visa",
            "type": "debit"
          },
          "transaction_identifier": "<TRANSACTION_ID_FROM_TOKEN>"
        }
      }
    },
    "billing": {
      "address": {
        "line1": "1 Infinite Loop",
        "city": "Cupertino",
        "state": "California",
        "zip": "95014",
        "country": "US",
        "first_name": "Test",
        "last_name": "User"
      }
    }
  }'
```

**Expected response:**
```json
{
  "status": "succeeded",
  "connector": "trustpayments",
  "whole_connector_response": {
    "requesttypedescription": "AUTH",
    "walletsource": "APPLEPAY",
    "walletdisplayname": "Visa 4242",
    "errorcode": "0",
    "errormessage": "Ok"
  }
}
```

**Last known good token:** Ask a dev to generate a fresh one encrypted with merchant ID `merchant.com.stripe.sang`.
(Tokens expire and should not be stored in version control.)

> **Note on tokens:** Apple Pay tokens expire. When a token no longer works, ask a dev to generate a new one.
> The `payment_method.display_name`, `network`, and `type` in the request body should reflect the card in the new token.

---

## 7. gRPC Tests — Direct to UCS (No HS Router)

Use these to validate UCS → TrustPayments in isolation (no decryption needed, you provide DPAN directly).

### Test 1: Visa 3DS — GBP
```bash
grpcurl -plaintext \
  -H "x-connector: trustpayments" \
  -H "x-auth: signature-key" \
  -H "x-merchant-id: merchant_finix_test_1" \
  -H 'x-api-key: ws@justpay.in' \
  -H 'x-key1: H!*!tm7C,ksm' \
  -H 'x-api-secret: test_juspay140213' \
  -d '{
    "amount": {"minor_amount": 1050, "currency": "GBP"},
    "payment_method": {
      "apple_pay": {
        "payment_data": {
          "decrypted_data": {
            "application_primary_account_number": {"value": "4111111111111111"},
            "application_expiration_month": {"value": "03"},
            "application_expiration_year": {"value": "2030"},
            "payment_data": {
              "online_payment_cryptogram": {"value": "AgAAAAAABk0LhJQ3QRgAAAAAAA"},
              "eci_indicator": "07"
            }
          }
        },
        "payment_method": {"display_name": "Visa 1111", "network": "VISA", "type": "CREDIT"},
        "transaction_identifier": "visa-gbp-test"
      }
    },
    "address": {"billing_address": {"first_name": {"value": "John"}, "last_name": {"value": "Doe"}, "line1": {"value": "123 Test St"}, "city": {"value": "London"}, "country_alpha2_code": "GB"}},
    "capture_method": "AUTOMATIC",
    "auth_type": "NO_THREE_DS",
    "test_mode": true
  }' \
  localhost:8000 types.PaymentService/Authorize
```
**Expected:** `status: CHARGED`, `eci: "07"`, `walletdisplayname: "Visa 1111"`, `tavv` present, `tokentype: "APPLEPAY"`

### Test 2: Mastercard — USD
```bash
grpcurl -plaintext \
  -H "x-connector: trustpayments" -H "x-auth: signature-key" -H "x-merchant-id: merchant_finix_test_1" \
  -H 'x-api-key: ws@justpay.in' -H 'x-key1: H!*!tm7C,ksm' -H 'x-api-secret: test_juspay140213' \
  -d '{
    "amount": {"minor_amount": 2500, "currency": "USD"},
    "payment_method": {
      "apple_pay": {
        "payment_data": {
          "decrypted_data": {
            "application_primary_account_number": {"value": "5555555555554444"},
            "application_expiration_month": {"value": "12"},
            "application_expiration_year": {"value": "2028"},
            "payment_data": {"online_payment_cryptogram": {"value": "AgAAAAAABk0LhJQ3QRgBBBBBBB"}, "eci_indicator": "02"}
          }
        },
        "payment_method": {"display_name": "MasterCard 4444", "network": "MASTERCARD", "type": "CREDIT"},
        "transaction_identifier": "mc-usd-test"
      }
    },
    "address": {"billing_address": {"first_name": {"value": "Jane"}, "last_name": {"value": "Smith"}, "line1": {"value": "456 Oak Ave"}, "city": {"value": "New York"}, "country_alpha2_code": "US"}},
    "capture_method": "AUTOMATIC", "auth_type": "THREE_DS", "test_mode": true
  }' \
  localhost:8000 types.PaymentService/Authorize
```
**Expected:** `status: CHARGED`, `eci: "02"`, `walletdisplayname: "MasterCard 4444"`

### Test 3: Amex — EUR
```bash
grpcurl -plaintext \
  -H "x-connector: trustpayments" -H "x-auth: signature-key" -H "x-merchant-id: merchant_finix_test_1" \
  -H 'x-api-key: ws@justpay.in' -H 'x-key1: H!*!tm7C,ksm' -H 'x-api-secret: test_juspay140213' \
  -d '{
    "amount": {"minor_amount": 9999, "currency": "EUR"},
    "payment_method": {
      "apple_pay": {
        "payment_data": {
          "decrypted_data": {
            "application_primary_account_number": {"value": "378282246310005"},
            "application_expiration_month": {"value": "09"},
            "application_expiration_year": {"value": "2027"},
            "payment_data": {"online_payment_cryptogram": {"value": "AmexCryptoTest1234567890AB="}, "eci_indicator": "05"}
          }
        },
        "payment_method": {"display_name": "Amex 0005", "network": "AMEX", "type": "CREDIT"},
        "transaction_identifier": "amex-eur-test"
      }
    },
    "address": {"billing_address": {"first_name": {"value": "Alice"}, "last_name": {"value": "Wonder"}, "line1": {"value": "1 Berliner Str"}, "city": {"value": "Berlin"}, "country_alpha2_code": "DE"}},
    "capture_method": "AUTOMATIC", "auth_type": "THREE_DS", "test_mode": true
  }' \
  localhost:8000 types.PaymentService/Authorize
```
**Expected:** `status: CHARGED`, `eci: "05"`, `walletdisplayname: "Amex 0005"`, `paymenttypedescription: "AMEX"`

### gRPC Test Results (2026-04-16)

| Card | DPAN | Currency | ECI | Result | Txn Ref |
|------|------|----------|-----|--------|---------|
| Visa | 4111111111111111 | GBP | 07 | CHARGED ✅ | 58-9-4837037 |
| Mastercard | 5555555555554444 | USD | 02 | CHARGED ✅ | 58-9-4837043 |
| Visa (no-3DS) | 4111111111111111 | GBP | 07 | CHARGED ✅ | 56-9-4837685 |
| Amex | 378282246310005 | EUR | 05 | CHARGED ✅ | 56-9-4837687 |

---

## 8. Code Change in PR #1046 (+ Our Fix)

**File:** `crates/integrations/connector-integration/src/connectors/trustpayments/transformers.rs`

The PR was missing `walletdisplayname` — a required TrustPayments field. We added it.

**Struct:**
```rust
pub struct TrustpaymentsApplePayData {
    pub pan: Secret<String>,
    pub expirydate: Secret<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tavv: Option<Secret<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub eci: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenisedpayment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokentype: Option<String>,
    pub walletdisplayname: String,   // <-- ADDED
    pub walletsource: String,
}
```

**Mapping:**
```rust
walletdisplayname: apple_pay_data.payment_method.display_name.clone(),  // <-- ADDED
```

**Stash reference** (UCS repo, `feat/grace-trustpayments-applepay`):
```bash
# Apply the fix:
git checkout feat/grace-trustpayments-applepay
git stash apply stash@{0}   # "feat(trustpayments): add walletdisplayname to ApplePay"
```

---

## 9. Troubleshooting

### "relative URL without a base" from UCS
**Cause:** MCA `metadata` is missing or not `apple_pay_combined/simplified`.
HS router is not decrypting — it's sending the encrypted token to UCS's Tokenize step, which has no URL configured for TrustPayments.
**Fix:** Set MCA metadata per Section 5c above.

### "failed to decrypt apple pay token" from HS Router
**Cause:** Apple Pay token was encrypted for a different PPC cert.
The token's `publicKeyHash` doesn't match our PPC cert (`IXgIwh5Oo2LtW5/lbld2SeQMyR483ofgeNEu+j4emeo=`).
**Fix:** Ask dev to generate a new token using the merchant ID `merchant.com.stripe.sang`.

### Check token's publicKeyHash
```bash
echo "<BASE64_TOKEN>" | base64 -d | python3 -c "import sys,json; d=json.load(sys.stdin); print('publicKeyHash:', d['header']['publicKeyHash'])"
```
Must match: `IXgIwh5Oo2LtW5/lbld2SeQMyR483ofgeNEu+j4emeo=`

### "connector not found" or payment goes to wrong connector
**Cause:** Routing is not set to TrustPayments.
**Fix:** Check and update routing per Section 5d.

### TrustPayments API returns errorcode != "0"
Check `whole_connector_response` in the payment response for `errormessage` details.

---

## 10. TrustPayments Request Fields Reference

| Field | Value | Source |
|-------|-------|--------|
| `requesttypedescriptions` | `["AUTH"]` | Hardcoded |
| `accounttypedescription` | `"ECOM"` | Hardcoded |
| `walletsource` | `"APPLEPAY"` | Hardcoded |
| `walletdisplayname` | e.g. `"Visa 1111"` | `payment_method.display_name` |
| `pan` | DPAN (16 digits) | `application_primary_account_number` |
| `expirydate` | `MM/YYYY` | Converted from Apple Pay month/year |
| `eci` | `"05"/"07"/"02"/"06"` | `eci_indicator` or default `"07"` |
| `tavv` | cryptogram string | `online_payment_cryptogram` (omitted if empty) |
| `tokenisedpayment` | `"1"` | Only when cryptogram present |
| `tokentype` | `"APPLEPAY"` | Only when cryptogram present |
| `sitereference` | `test_juspay140213` | From auth credentials |
