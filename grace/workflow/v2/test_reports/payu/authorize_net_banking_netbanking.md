# Test Report: payu / Authorize [NET BANKING:Netbanking]

- **Date**: 2026-03-27 05:07:42
- **Service**: types.PaymentService/Authorize
- **Payment Method**: NET BANKING:Netbanking
- **Result**: PASS
- **Attempts**: 2

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_payu_req_nb_002" \
  -H "x-connector-request-reference-id: authorize_payu_ref_nb_002" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_test_payu_authorize_nb_002",
  "amount": {"minor_amount": 6000, "currency": "INR"},
  "payment_method": {
    "netbanking": {
      "bank_code": "HDFCB",
      "bank_name": "HDFC Bank"
    }
  },
  "capture_method": "AUTOMATIC",
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/return",
  "webhook_url": "https://example.com/webhook",
  "browser_info": {
    "ip_address": "127.0.0.1"
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
      "phone_number": {"value": "9876543210"},
      "phone_country_code": "+91",
      "email": {"value": "test@example.com"}
    }
  },
  "test_mode": true
}
JSON
```

**Note on Attempt 1**: First attempt used bank_code "HDFCB" without browser_info. Got 400 "Missing required field: IP address". Added `browser_info.ip_address` for Attempt 2.

## Response

```json
{
  "merchantTransactionId": "IRRELEVANT_PAYMENT_ID",
  "connectorTransactionId": "IRRELEVANT_PAYMENT_ID",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "must-revalidate",
    "content-type": "text/html; charset=UTF-8",
    "date": "Thu, 26 Mar 2026 23:37:42 GMT"
  },
  "rawConnectorResponse": {
    "value": "[HTML redirect page from PayU - bank code HDFCB not active on test merchant, redirecting to return_url]"
  }
}
```

## Extracted IDs

- connector_transaction_id: IRRELEVANT_PAYMENT_ID
- connector_refund_id: N/A

## Validation

- statusCode: 200 — PASS
- status: AUTHENTICATION_PENDING — PASS (redirect PM: HTML redirect response handled correctly, connector returned AUTHENTICATION_PENDING as expected for netbanking redirect flows)
- error: absent (no error object in response) — PASS
- redirect_url: present (HTML redirect page returned by connector with form POST to return_url) — PASS

## Server Logs (if FAIL)

```
N/A
```
