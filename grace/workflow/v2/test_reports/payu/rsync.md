# Test Report: payu / RSync

- **Date**: 2026-03-27 00:00:00
- **Service**: types.RefundService/Get
- **Payment Method**: N/A
- **Result**: PASS
- **Attempts**: 2

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: rsync_payu_req_001" \
  -H "x-connector-request-reference-id: rsync_payu_ref_001" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "403993715537077287",
  "refund_id": "403993715537077287",
  "test_mode": true
}
JSON
```

## Response

```json
{
  "error": {
    "connectorDetails": {
      "code": "PAYU_RSYNC_ERROR",
      "message": "0 out of 1 Transactions Fetched Successfully"
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "*",
    "access-control-allow-methods": "*",
    "connection": "keep-alive",
    "content-length": "156",
    "content-security-policy": "default-src 'self'",
    "content-type": "application/json",
    "date": "Thu, 26 Mar 2026 23:59:23 GMT",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "vary": "Origin",
    "x-content-type-options": "nosniff",
    "x-dns-prefetch-control": "off",
    "x-download-options": "noopen",
    "x-frame-options": "SAMEORIGIN",
    "x-xss-protection": "1; mode=block"
  },
  "rawConnectorResponse": {
    "value": "{\"status\":0,\"msg\":\"0 out of 1 Transactions Fetched Successfully\",\"transaction_details\":{\"403993715537077287\":{\"mihpayid\":\"Not Found\",\"status\":\"Not Found\"}}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://test.payu.in//merchant/postservice.php?form=2\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/x-www-form-urlencoded\",\"Accept\":\"application/json\"},\"body\":\"key=T58CQx&command=verify_payment&var1=403993715537077287&hash=f64d7a3fd87fc7e987154e109c83a04890ff2eb29f4a87cfc8ae8eda42d199e96ccecbd940d1b6de85678cce541f1d87013f60d9df4646fdcf99d88242981d2d\"}"
  }
}
```

## Extracted IDs

- connector_transaction_id: 403993715537077287
- connector_refund_id: 403993715537077287

## Validation

- statusCode: 200 — PASS
- status: N/A (connector returned "Not Found" for refund in test environment) — PASS (per parent note: statusCode 200 + correct deserialization = PASS)
- error: present (connector-level PAYU_RSYNC_ERROR: refund not found in test env) — noted; deserialization bug was pre-fixed
- Attempt 1: types.PaymentService/RSync — FAIL (method not found on PaymentService)
- Attempt 2: types.RefundService/Get — PASS (200, correct deserialization)

## Notes

- First attempt used `types.PaymentService/RSync` which does not exist; corrected to `types.RefundService/Get`.
- PayU's test environment returned "Not Found" for the refund ID, which is a connector/data-level response, not a code bug.
- The PayuRefundSyncResponse deserialization bug was pre-fixed; the response deserializes correctly at statusCode 200.
- Per parent instructions: "A response with statusCode 200 that deserializes correctly counts as PASS."

## Server Logs (if FAIL)

```
N/A
```
