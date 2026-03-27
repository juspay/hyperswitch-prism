# Test Report: payu / Authorize [UPI:UPI_QR]

- **Date**: 2026-03-27 04:25:01
- **Service**: types.PaymentService/Authorize
- **Payment Method**: UPI:UPI_QR
- **Result**: PASS
- **Attempts**: 1

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_payu_req_upi_qr" \
  -H "x-connector-request-reference-id: authorize_payu_ref_upi_qr" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_test_payu_authorize_upi_qr_001",
  "amount": {"minor_amount": 6000, "currency": "INR"},
  "payment_method": {"upi_qr": {}},
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
  "merchantTransactionId": "403993715537077211",
  "connectorTransactionId": "403993715537077211",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "must-revalidate",
    "connection": "keep-alive",
    "content-security-policy": "object-src 'none'; img-src https: data: *.payubiz.in *.payu.in  *.payumoney.com; frame-ancestors 'self' *.facebook.com *.odoo.com payments.np.flydubai.com *.instagram.com *.meta.com *.myshopify.com pre-qa.samsung.com pre-qa2.samsung.com www.samsung.com 3ds2-ui-acsuat.pc.enstage-sas.com;",
    "content-type": "text/html; charset=UTF-8",
    "date": "Thu, 26 Mar 2026 22:55:01 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "p3p": "CP=\"IDC DSP COR ADM DEVi TAIi PSA PSD IVAi IVDi CONi HIS OUR IND CNT\"",
    "pragma": "no-cache",
    "set-cookie": "PHPSESSID=69c5b94582597; secure; HttpOnly;HttpOnly;Secure;SameSite=lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "vary": "Origin",
    "x-content-type-options": "nosniff",
    "x-frame-options": "sameorigin allow-from https://www.payumoney.com; ..."
  },
  "redirectionData": {
    "uri": {
      "uri": "pa=kk.payutest@hdfcbank&pn=juspay in&tr=403993715537077211&tid=PPPL4039937155370772112703260425016&am=60.00&cu=INR&tn=UPIIntent&split=CCONFEE:0.28"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"status\":1,\"token\":\"1A9E0AC4-28C7-706A-3778-AED5B585A9F6\",\"referenceId\":\"403993715537077211\",\"returnUrl\":\"https://test.payu.in/a6d5a4bc1ea521607abfad49501b2dd2ed0e31703b89ff1642d644b6cd30b9d8/genericIntentResponse.php\",\"merchantName\":\"juspay in\",\"merchantVpa\":\"kk.payutest@hdfcbank\",\"amount\":\"60.00\",\"intentSdkCombineVerifyAndPayButton\":null,\"txnId\":\"mti_test_payu_authorize_upi_qr_001\",\"disableIntentSeamlessFailure\":\"0\",\"vpaRegex\":\"/^[a-zA-Z0-9.-]+@[a-zA-Z0-9.-]+$/\",\"apps\":[...],\"upiPushDisabled\":\"0\",\"pushServiceUrl\":\"https://nimtestint.payu.in/upi/secureVerify?encId=403993715537077211...\",\"intentURIData\":\"pa=kk.payutest@hdfcbank&pn=juspay in&tr=403993715537077211&tid=PPPL4039937155370772112703260425016&am=60.00&cu=INR&tn=UPIIntent&split=CCONFEE:0.28\"}"
  }
}
```

## Extracted IDs

- connector_transaction_id: 403993715537077211
- connector_refund_id: N/A

## Validation

- statusCode: 200 — PASS
- status: AUTHENTICATION_PENDING — PASS (UPI QR intent flow; connector returned intent URI in redirectionData confirming successful authorization initiation)
- error: absent — PASS
- redirect_url/intent_uri: present (pa=kk.payutest@hdfcbank&pn=juspay in&tr=403993715537077211...) — PASS

## Server Logs (if FAIL)

N/A
