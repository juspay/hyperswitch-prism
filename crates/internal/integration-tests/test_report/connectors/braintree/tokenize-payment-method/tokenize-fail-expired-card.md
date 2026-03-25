# Connector `braintree` / Suite `tokenize_payment_method` / Scenario `tokenize_fail_expired_card`

- Service: `Unknown`
- PM / PMT: `card` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to exist
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_fail_expired_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_expired_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_479215",
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
        "value": "01"
      },
      "card_exp_year": {
        "value": "2020"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Expired Card"
      }
    }
  },
  "customer": {
    "id": "cust_3848298e00aa4bfe902d773b",
    "name": "Liam Taylor",
    "email": {
      "value": "alex.4847@example.com"
    }
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
x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_expired_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_fail_expired_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:47:46 GMT
x-request-id: tokenize_payment_method_tokenize_fail_expired_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b2476d823ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:47:46 GMT",
    "paypal-debug-id": "25cb385f566d4",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=XrI_9nu_siWzMq6I92g6Ro9Q8E6zbbSJ7M8x6FieqwQ-1774324066.46226-1.0.1.1-ocuKZ_eWwpKlfP3svj8mlrtjPvbNHa1kqJ.upQ8yZgOGKnzcczPtMrvzFe7WNH.Ujr.bXquXoKy.OmZFDBQDXXNAWqOTbmLwrIfyVZUWaH7F_CWD1LcYh6Fzo10pUIyU; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:17:46 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "merchantPaymentMethodId": "tokencc_bh_3gvs2g_zfbp6h_22kb35_m8hnzy_mv5"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)
