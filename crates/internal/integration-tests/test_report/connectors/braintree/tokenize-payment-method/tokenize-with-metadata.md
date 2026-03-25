# Connector `braintree` / Suite `tokenize_payment_method` / Scenario `tokenize_with_metadata`

- Service: `Unknown`
- PM / PMT: `card` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_with_metadata_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_with_metadata_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_672601",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "2029"
      },
      "card_cvc": ***MASKED***
        "value": "789"
      },
      "card_holder_name": {
        "value": "Test User"
      }
    }
  },
  "customer": {
    "id": "cust_94b5ffa439ee4f0f8064bb23",
    "name": "Mia Wilson",
    "email": {
      "value": "jordan.1543@example.com"
    }
  },
  "metadata": {
    "value": "{\"source\":\"mobile\",\"device_id\":\"test-device-123\"}"
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_with_metadata_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_with_metadata_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:47:47 GMT
x-request-id: tokenize_payment_method_tokenize_with_metadata_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b24d595b3ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:47:47 GMT",
    "paypal-debug-id": "3b0a1a30bf7cc",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=6ty_Yr95cJUMBjTn8dNQdjSQNXDBaD.ViUhUGcvK3P8-1774324067.42003-1.0.1.1-HZcIlbqlXnMo3RmBXDl8UJ3bLCG7hThAmbdaIUDkYpRPgjFFXcRKUSNTZGRFSJQwXg3LN1Icy10JRX6TyA_1LqvnHbY9SaW0Eirn5mtut6pD8TyM4u4nOtEXBzR3LjIo; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:17:47 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "merchantPaymentMethodId": "tokencc_bh_zjgrrk_vvk3zg_xdjn9x_gnz8f3_kp4"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)
