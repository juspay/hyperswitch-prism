# Connector `braintree` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. tokenize_payment_method(tokenize_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_credit_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_713943",
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
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "John Doe"
      }
    }
  },
  "customer": {
    "id": "cust_c08c0be40c1640a2927d3576",
    "name": "Liam Miller",
    "email": {
      "value": "riley.1657@example.com"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4484 Sunset Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45472"
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
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:48:05 GMT
x-request-id: tokenize_payment_method_tokenize_credit_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b2befc013ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:48:05 GMT",
    "paypal-debug-id": "7395595be08c9",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=BHFKoZeDeCWRMntVSnOhPMbc5cXvFSl2hHLdr8zl9oY-1774324085.5956495-1.0.1.1-_KntJBKf1BelnHZf7lMyP7jUVbufVmTlXlgj3d5ikTxpFak_7PLeR1SgO1Eb9oiXQbBWIc8MfOJxlSFBE2GAwJW2zH53jJF5G7RHoUpe3zLVu6WyZte6Fw5LxRWbvpQt; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:18:05 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "merchantPaymentMethodId": "tokencc_bh_346rrt_5cq4gn_s3drb6_bzccwg_3h5"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_46cfb3ba740d49c0bb5abd26",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "John Doe"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Miller",
    "email": {
      "value": "riley.1657@example.com"
    },
    "id": "cust_c08c0be40c1640a2927d3576",
    "phone_number": "+13515378138"
  },
  "payment_method_token": ***MASKED***
    "value": "tokencc_bh_346rrt_5cq4gn_s3drb6_bzccwg_3h5"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "6825 Lake Rd"
      },
      "line2": {
        "value": "1618 Sunset Dr"
      },
      "line3": {
        "value": "7222 Oak Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39387"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.6063@testmail.io"
      },
      "phone_number": {
        "value": "7279332890"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4484 Sunset Blvd"
      },
      "line2": {
        "value": "9535 Market Blvd"
      },
      "line3": {
        "value": "426 Oak St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45472"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9803@testmail.io"
      },
      "phone_number": {
        "value": "8106200081"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  },
  "connector_feature_data": {
    "value": "{\"merchant_account_id\":\"juspay\",\"merchant_config_currency\":\"USD\"}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:48:06 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b2c16d983ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:48:06 GMT",
    "paypal-debug-id": "a8e887ca257a2",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=krRb4_jBpjEo_cpJgHL.K3R_gTm0FjllUkBeT.hjSjQ-1774324085.9869645-1.0.1.1-xT5IzFlgNbwsrAjoLOUlA4g0c.FgkpszYsNCMIO2t7hgbK06nEEqvK7t9XKU2N_f1zSBQd6w4PEWC709tw7LdkpwNYhxayH4O53t.AAwN7hUW7IvtvyJxiMbuBIRg.HR; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:18:06 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "redirectionData": {
    "braintree": {
      "clientToken": ***MASKED***"
      "cardToken": ***MASKED***"
      "bin": "424242",
      "acsUrl": "https://example.com/payment/complete"
    }
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
