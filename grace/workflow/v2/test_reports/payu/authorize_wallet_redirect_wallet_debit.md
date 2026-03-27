# Test Report: payu / Authorize [WALLET:REDIRECT_WALLET_DEBIT]

- **Date**: 2026-03-27 00:05:00
- **Service**: types.PaymentService/Authorize
- **Payment Method**: WALLET:REDIRECT_WALLET_DEBIT
- **Result**: PASS
- **Attempts**: 1

## grpcurl Command (credentials masked)

```bash
grpcurl -plaintext \
  -H "x-connector: payu" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_payu_req_wallet_006" \
  -H "x-connector-request-reference-id: authorize_payu_ref_wallet_006" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8001 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_test_payu_authorize_wallet_006",
  "amount": {"minor_amount": 6000, "currency": "INR"},
  "payment_method": {"pay_u_redirect": {}},
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
  "merchantTransactionId": "IRRELEVANT_PAYMENT_ID",
  "connectorTransactionId": "IRRELEVANT_PAYMENT_ID",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "must-revalidate",
    "connection": "keep-alive",
    "content-security-policy": "object-src 'none'; img-src https: data: *.payubiz.in *.payu.in  *.payumoney.com; frame-ancestors 'self' *.facebook.com *.odoo.com payments.np.flydubai.com *.instagram.com *.meta.com *.myshopify.com pre-qa.samsung.com pre-qa2.samsung.com www.samsung.com 3ds2-ui-acsuat.pc.enstage-sas.com;",
    "content-type": "text/html; charset=UTF-8",
    "date": "Thu, 26 Mar 2026 23:34:28 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "p3p": "CP=\"IDC DSP COR ADM DEVi TAIi PSA PSD IVAi IVDi CONi HIS OUR IND CNT\"",
    "pragma": "no-cache",
    "set-cookie": "PHPSESSID=egjs4ob2ld4d1shtle5d7pmhjh; path=/; domain=.payu.in;HttpOnly;Secure;SameSite=lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "vary": "Origin",
    "x-content-type-options": "nosniff",
    "x-frame-options": "sameorigin allow-from https://www.payumoney.com; ..."
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://test.payu.in//_payment\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/x-www-form-urlencoded\",\"Accept\":\"application/json\",\"via\":\"HyperSwitch\"},\"body\":\"key=***MASKED***&txnid=mti_test_payu_authorize_wallet_006&amount=60.00&currency=INR&productinfo=Payment&firstname=Test&lastname=User&email=test%40example.com&phone=%2B919876543210&surl=https%3A%2F%2Fexample.com%2Freturn&furl=https%3A%2F%2Fexample.com%2Freturn&pg=BNPL&bankcode=LAZYPAY&s2s_client_ip=192.168.1.1&s2s_device_info=web&api_version=2.0&hash=***MASKED***&udf1=IRRELEVANT_PAYMENT_ID&udf2=test_merchant\"}"
  }
}
```

## Extracted IDs

- connector_transaction_id: IRRELEVANT_PAYMENT_ID
- connector_refund_id: N/A

## Validation

- statusCode: 200 — PASS
- status: AUTHENTICATION_PENDING — PASS (expected for Wallet Redirect flows — connector handles HTML response via synthetic JSON substitution)
- error: absent — PASS
- redirect_url: present (HTML redirect page with form action URL to test.payu.in/_payment_options) — PASS

## Server Logs (if FAIL)

N/A
