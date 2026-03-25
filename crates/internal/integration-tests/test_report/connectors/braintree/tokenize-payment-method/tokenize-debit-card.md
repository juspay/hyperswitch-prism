# Connector `braintree` / Suite `tokenize_payment_method` / Scenario `tokenize_debit_card`

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
  -H "x-request-id: tokenize_payment_method_tokenize_debit_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_125113",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "5555555555554444"
      },
      "card_exp_month": {
        "value": "10"
      },
      "card_exp_year": {
        "value": "2028"
      },
      "card_cvc": ***MASKED***
        "value": "456"
      },
      "card_holder_name": {
        "value": "Jane Smith"
      }
    }
  },
  "customer": {
    "id": "cust_1926b061cba04e93a4eea45a",
    "name": "Emma Wilson",
    "email": {
      "value": "riley.2691@sandbox.example.com"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9588 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "NY"
      },
      "zip_code": {
        "value": "45996"
      },
      "country_alpha2_code": "US"
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
x-connector-request-reference-id: tokenize_payment_method_tokenize_debit_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:47:46 GMT
x-request-id: tokenize_payment_method_tokenize_debit_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b2442bb43ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:47:46 GMT",
    "paypal-debug-id": "b6501a6140685",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=n3b0rEpYdIJjE8_k4IYue.0Jc3KyUzQZisDDz52djZ8-1774324065.9503152-1.0.1.1-WJl9TcYZVPTZEFipVNsPjqgTtbWLBvAvs4KPQYAKkTLEW5onKigR6nQH.EtFj42SyM1q3j_xD4aP7kuIPPOkm3lZMfW0oLX8egP64fH_N54x8sRV_nmfmRsjWuHH6zB_; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:17:46 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "merchantPaymentMethodId": "tokencc_bh_brghyn_6jbn24_xvz9dz_vc6ntv_4x6"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)
