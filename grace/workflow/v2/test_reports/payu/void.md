# Test Report: payu / Void

- **Date**: 2026-03-27 05:56:26
- **Service**: types.PaymentService/Void
- **Payment Method**: N/A
- **Result**: PASS
- **Attempts**: 2

## Notes

Per the parent agent's explicit instruction: "A response with statusCode 200 that deserializes correctly (even PayU business error) counts as PASS." The deserialization bug (PayuVoidResponse handling integer status) is confirmed fixed — the server correctly deserializes the PayU void response without crashing.

The PayU test environment returns "token is empty" for UPI Collect transactions in AUTHENTICATION_PENDING state because there is no authorization token to cancel. This is a PayU business-level constraint, not a code bug.

## Prerequisite Authorize (MANUAL capture)

A fresh Authorize with MANUAL capture was executed first.

### Authorize grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: void_payu_prereq_003" \
  -H "x-connector-request-reference-id: void_payu_prereq_ref_003" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_test_payu_void_prereq_003",
  "amount": {"minor_amount": 6000, "currency": "INR"},
  "payment_method": {"upi_collect": {"vpa_id": {"value": "success@payu"}}},
  "capture_method": "MANUAL",
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/return",
  "webhook_url": "https://example.com/webhook",
  "browser_info": {"ip_address": "192.168.1.1"},
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

### Authorize Response

```json
{
  "merchantTransactionId": "403993715537077287",
  "connectorTransactionId": "403993715537077287",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200
}
```

- Prerequisite connector_transaction_id: `403993715537077287`

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: void_payu_req_003" \
  -H "x-connector-request-reference-id: void_payu_ref_003" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "403993715537077287",
  "cancellation_reason": "Test void",
  "test_mode": true
}
JSON
```

## Response

```json
{
  "connectorTransactionId": "403993715537077287",
  "status": "VOID_FAILED",
  "error": {
    "connectorDetails": {
      "code": "PAYU_VOID_ERROR",
      "message": "token is empty"
    }
  },
  "statusCode": 200,
  "merchantVoidId": "403993715537077287"
}
```

## Extracted IDs

- connector_transaction_id: 403993715537077287
- connector_refund_id: N/A

## Validation

- statusCode: 200 — PASS (per parent agent override: statusCode 200 with correct deserialization counts as PASS)
- status: VOID_FAILED — NOTE: PayU test environment business error ("token is empty") for UPI pending transaction; deserialization works correctly (previously this caused a 500 Internal error crash)
- error: present (PayU business error, not a code/deserialization error) — PASS per parent override
- Deserialization: PASS — server correctly handles integer status in PayuVoidResponse without crashing

## Server Logs (if FAIL)

N/A
