# Test Report: payu / Refund

- **Date**: 2026-03-27 00:00:00
- **Service**: types.PaymentService/Refund
- **Payment Method**: N/A
- **Result**: PASS
- **Attempts**: 1

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_payu_req_001" \
  -H "x-connector-request-reference-id: refund_payu_ref_001" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_test_payu_refund_001",
  "connector_transaction_id": "403993715537077287",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "INR"
  },
  "test_mode": true
}
JSON
```

## Response

```json
{
  "connectorRefundId": "403993715537077287",
  "status": "REFUND_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "must-revalidate",
    "connection": "keep-alive",
    "content-security-policy": "object-src 'none'; img-src https: data: *.payubiz.in *.payu.in  *.payumoney.com; frame-ancestors 'self' *.facebook.com *.odoo.com payments.np.flydubai.com *.instagram.com *.meta.com *.myshopify.com pre-qa.samsung.com pre-qa2.samsung.com www.samsung.com 3ds2-ui-acsuat.pc.enstage-sas.com;",
    "content-type": "text/html; charset=UTF-8",
    "date": "Thu, 26 Mar 2026 23:58:18 GMT",
    "p3p": "CP=\"IDC DSP COR ADM DEVi TAIi PSA PSD IVAi IVDi CONi HIS OUR IND CNT\"",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "vary": "Origin",
    "x-content-type-options": "nosniff",
    "x-frame-options": "sameorigin allow-from https://www.payumoney.com; https://www.goibibobusiness.com; https://www.premiermiles.co.in; https://goibibo.com; https://secure.skype.com; https://www.facebook.com; https://api.payu.in; https://payments.np.flydubai.com; https://apitest.payu.in; https://testtxncdn.payubiz.in; https://3ds2-ui-acsuat.pc.enstage-sas.com"
  },
  "connectorTransactionId": "403993715537077287",
  "rawConnectorResponse": {
    "value": "{\"status\":236,\"msg\":\"Refund Split Info must be of JSON format\",\"mihpayid\":\"403993715537077287\"}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://test.payu.in//merchant/postservice.php?form=2\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/x-www-form-urlencoded\",\"Accept\":\"application/json\"},\"body\":\"key=***MASKED***&command=cancel_refund_transaction&var1=403993715537077287&var2=60.00&var3=mri_test_payu_refund_001&hash=***MASKED***\"}"
  }
}
```

## Extracted IDs

- connector_transaction_id: 403993715537077287
- connector_refund_id: 403993715537077287

## Validation

- statusCode: 200 — PASS
- status: REFUND_PENDING — PASS
- error: absent — PASS
- redirect_url: N/A

## Notes

The PayU connector returned a business-level response with status code 236 ("Refund Split Info must be of JSON format") in the raw connector response. However, the gRPC response correctly deserialized with statusCode 200 and status REFUND_PENDING. Per the test notes, "A response with statusCode 200 that deserializes correctly (even PayU business error) counts as PASS — the deserialization bug for PayuRefundResponse was already fixed."

## Server Logs (if FAIL)

```
N/A
```
