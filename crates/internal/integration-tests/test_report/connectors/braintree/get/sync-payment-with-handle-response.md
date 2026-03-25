# Connector `braintree` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Error invoking method "types.PaymentService/Get": error getting request data: message type types.PaymentServiceGetRequest has no known field named payment_method_token
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
  "merchant_payment_method_id": "gen_586664",
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
    "id": "cust_c8f96127bc1d449da88a1933",
    "name": "Ethan Miller",
    "email": {
      "value": "casey.1373@testmail.io"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1102 Pine St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60785"
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
date: Tue, 24 Mar 2026 03:20:40 GMT
x-request-id: tokenize_payment_method_tokenize_credit_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e128a91bb293bd0-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:20:40 GMT",
    "paypal-debug-id": "bcd01731abcde",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=scEqgA7bT.fxHOQjwMFD1V_wk0JAGnRm_JFjvxjHVYg-1774322439.9600298-1.0.1.1-hxYkpLPbQ_JuD.g8AXX8KnMLGrARtluTe80l8lPhVHlUymMITtDDcWhE.xHzNQz2YLkK4l_yBw3enFC4ch8aH_41UyZ21qjURgbVfMr4H0ZNyOSGV.0RpiN7hL6PMnEv; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 03:50:40 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "merchantPaymentMethodId": "tokencc_bh_9jgd4n_398nfk_vfk4v3_kv8387_j85"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_42a91ce7f42c44a0b64c066a",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "casey.1373@testmail.io"
    },
    "id": "cust_c8f96127bc1d449da88a1933",
    "phone_number": "+449922555199"
  },
  "payment_method_token": ***MASKED***
    "value": "tokencc_bh_9jgd4n_398nfk_vfk4v3_kv8387_j85"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9416 Market Blvd"
      },
      "line2": {
        "value": "423 Oak Rd"
      },
      "line3": {
        "value": "5048 Market St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41135"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7294@sandbox.example.com"
      },
      "phone_number": {
        "value": "8708557246"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1102 Pine St"
      },
      "line2": {
        "value": "2518 Sunset Ave"
      },
      "line3": {
        "value": "6701 Market Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60785"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2076@sandbox.example.com"
      },
      "phone_number": {
        "value": "1976958149"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "No3DS auto capture card payment (credit)",
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
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:20:41 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "dHJhbnNhY3Rpb25fZ2Y5ZTkzdzE",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e128a93bb993bd0-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:20:41 GMT",
    "paypal-debug-id": "ac0d4840e7bfd",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=18pAeZJpMRoX1Hwu3ofzkJnuVsouB8H3LL9zto2A0II-1774322440.2798693-1.0.1.1-DlBoXatZaRL16bt2HFF6W9lc69mS3dNFxz2l8v0sgBBoOMBz4vldChlURvPH02DpLu7KRRRIV6fbePLnsLuaeHtFrCBJpFEHszQ68oCO1CbTjvdTNda4Cl9FTgrVVBxr; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 03:50:41 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "dHJhbnNhY3Rpb25fZ2Y5ZTkzdzE",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method_token": ***MASKED***
    "value": "tokencc_bh_9jgd4n_398nfk_vfk4v3_kv8387_j85"
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
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Error invoking method "types.PaymentService/Get": error getting request data: message type types.PaymentServiceGetRequest has no known field named payment_method_token
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
