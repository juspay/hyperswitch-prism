# Test Report: payu / Authorize [UPI:UPI_COLLECT]

- **Date**: 2026-03-27 00:00:22
- **Service**: types.PaymentService/Authorize
- **Payment Method**: UPI:UPI_COLLECT
- **Result**: PASS
- **Attempts**: 2

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_payu_req_upi_collect_002" \
  -H "x-connector-request-reference-id: authorize_payu_ref_upi_collect_002" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_test_payu_authorize_upi_collect_002",
  "amount": {"minor_amount": 6000, "currency": "INR"},
  "payment_method": {"upi_collect": {"vpa_id": {"value": "success@payu"}}},
  "capture_method": "AUTOMATIC",
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

## Response

```json
{
  "merchantTransactionId": "403993715537077216",
  "connectorTransactionId": "403993715537077216",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "must-revalidate",
    "connection": "keep-alive",
    "content-type": "text/html; charset=UTF-8",
    "date": "Thu, 26 Mar 2026 23:00:22 GMT"
  },
  "rawConnectorResponse": {
    "value": "eyJzdGF0dXMiOiJzdWNjZXNzIiwicmVzdWx0Ijp7Im1paHBheWlkIjoiNDAzOTkzNzE1NTM3MDc3MjE2Iiw..."
  }
}
```

## Extracted IDs

- connector_transaction_id: 403993715537077216
- connector_refund_id: N/A

## Validation

- statusCode: 200 — PASS
- status: AUTHENTICATION_PENDING (maps to PENDING) — PASS
- error: absent — PASS
- redirect_url: N/A (UPI Collect is a direct PM, not redirect)

## Attempt Notes

- Attempt 1: Used VPA `test@upi` — FAIL (connector returned E6002: Invalid vpa)
- Attempt 2: Used VPA `success@payu` (PayU test VPA) — PASS

## Server Logs (if FAIL)

```
N/A
```
