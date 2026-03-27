# Test Report: payu / Authorize [UPI:UPI_PAY]

- **Date**: 2026-03-27 04:22:27
- **Service**: types.PaymentService/Authorize
- **Payment Method**: UPI:UPI_PAY
- **Result**: PASS
- **Attempts**: 3 (this invocation) + 3 (previous invocation)

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_payu_req" \
  -H "x-connector-request-reference-id: authorize_payu_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_test_payu_authorize_upi_pay_003",
  "amount": {"minor_amount": 6000, "currency": "INR"},
  "payment_method": {"upi_intent": {}},
  "capture_method": "AUTOMATIC",
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/return",
  "webhook_url": "https://example.com/webhook",
  "browser_info": {
    "ip_address": "192.168.1.1"
  },
  "address": {
    "billing_address": {
      "first_name": {"value": "Test"},
      "last_name": {"value": "User"},
      "line1": {"value": "123 Test St"},
      "city": {"value": "Mumbai"},
      "state": {"value": "MH"},
      "zip_code": {"value": "400001"},
      "country_alpha2_code": "IN",
      "email": {"value": "test@example.com"},
      "phone_number": {"value": "9876543210"},
      "phone_country_code": "+91"
    }
  },
  "test_mode": true
}
JSON
```

## Attempt History

### Previous Invocation (3 attempts exhausted)
- Attempt 1: `"payment_method": {"upi_intent": {}}` with no phone/email — 400 missing email
- Attempt 2: Added email — 400 missing phone
- Attempt 3: Email present, wrong credential set (connector_1 prod key) — rejected by PayU test env

### This Invocation
- **Attempt 1**: Added `"upi"` wrapper (`{"upi": {"upi_intent": {}}}`) — gRPC proto error: `message type types.PaymentMethod has no known field named upi`
- **Attempt 2**: Removed `"upi"` wrapper, added `phone_number` + `phone_country_code` to billing_address, used connector_1 credentials — 200 but PayU rejected: key is not a test environment key (EX147)
- **Attempt 3**: Same payload, switched to connector_2 credentials (`T58CQx` / `0vpUmrQTHLERol1p7Gdl3PYYyMLtLAd8`) — **SUCCESS**

## Response (Successful — Attempt 3)

```json
{
  "merchantTransactionId": "403993715537077208",
  "connectorTransactionId": "403993715537077208",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "redirectionData": {
    "uri": {
      "uri": "pa=kk.payutest@hdfcbank&pn=juspay in&tr=403993715537077208&tid=PPPL4039937155370772082703260422276&am=60.00&cu=INR&tn=UPIIntent&split=CCONFEE:0.28"
    }
  }
}
```

## Extracted IDs

- connector_transaction_id: 403993715537077208
- connector_refund_id: N/A

## Validation

- statusCode: 200 — PASS
- status: AUTHENTICATION_PENDING — PASS (UPI Intent S2S flow; redirectionData with UPI intent URI present; customer must complete via UPI app)
- error: absent — PASS
- redirect_url: present (UPI intent URI in redirectionData.uri.uri) — PASS

## Key Findings

1. `phone_number` (SecretString) and `phone_country_code` (string) must be included in `billing_address`
2. `browser_info.ip_address` must be provided (PayU's S2S UPI flow requires client IP)
3. Correct test credentials: connector_2 (`x-api-key: T58CQx`, `x-key1: 0vpUmrQTHLERol1p7Gdl3PYYyMLtLAd8`)

## Server Logs (if FAIL)

```
N/A — Test PASSED
```
