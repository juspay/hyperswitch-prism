# Connector `braintree` / Suite `authorize` / Scenario `no3ds_auto_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
- Result: `PASS`

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
  "merchant_payment_method_id": "gen_531939",
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
    "id": "cust_c1cddefb1cbe48669a86a157",
    "name": "Ava Johnson",
    "email": {
      "value": "morgan.5889@example.com"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "8024 Pine Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31576"
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
date: Tue, 24 Mar 2026 03:47:53 GMT
x-request-id: tokenize_payment_method_tokenize_credit_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b2751b503ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:47:53 GMT",
    "paypal-debug-id": "9d54eeb648bd9",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=kJI0JYjfHaqZAsqF6H4HFwnWwZxWZsllawoNPQZKRFM-1774324073.7725172-1.0.1.1-tXLo7FDogZuBPqINpZo2LOy0zv8Zu9nmoBDyS4owLM16SNidNRv4VHSHfXOgaPgHx.mP6ne1l8K2sr787U0V_JVpfUuwaMiSh5FhdUj5ttm3Y.vnvm.FyWGEPCrvQMr9; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:17:53 GMT",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Braintree-Version, Accept-Encoding",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY"
  },
  "merchantPaymentMethodId": "tokencc_bh_xsgp3k_3hfb4g_fwwzcb_yzyvtx_qx5"
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
  -H "x-request-id: authorize_no3ds_auto_capture_debit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_265c5cf94c424857bf205a10",
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
      "card_type": "debit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Johnson",
    "email": {
      "value": "morgan.5889@example.com"
    },
    "id": "cust_c1cddefb1cbe48669a86a157",
    "phone_number": "+16897186286"
  },
  "payment_method_token": ***MASKED***
    "value": "tokencc_bh_xsgp3k_3hfb4g_fwwzcb_yzyvtx_qx5"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9923 Main Ln"
      },
      "line2": {
        "value": "1735 Oak St"
      },
      "line3": {
        "value": "3182 Main Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52138"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9339@sandbox.example.com"
      },
      "phone_number": {
        "value": "7020284619"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "8024 Pine Dr"
      },
      "line2": {
        "value": "4831 Lake Rd"
      },
      "line3": {
        "value": "3538 Oak St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31576"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7467@testmail.io"
      },
      "phone_number": {
        "value": "3930626307"
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
  "description": "No3DS auto capture card payment (debit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:47:55 GMT
x-request-id: authorize_no3ds_auto_capture_debit_card_req

Response contents:
{
  "connectorTransactionId": "dHJhbnNhY3Rpb25fYnNhdjFwM3Y",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "braintree-version": "2016-10-07",
    "cache-control": "no-cache, no-store",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e12b277bd533ba9-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 03:47:55 GMT",
    "paypal-debug-id": "7a8bf1e65b5fc",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=NLhA46wVroK.YBVUOLqH1l6WOtnUXkr.65V4W3ffaXc-1774324074.1979556-1.0.1.1-1HFe_ideHns.8ifAM7BtF0mtUqe6IMqS60yS2OhwimyMCHZmHXaZPfsGEAeyVEIybXCgRiHYeT0q8wZlkzZ6Mo993U1mo111EbIy8QWY3PVkJFu6MifJWMx7C_eXBz32; HttpOnly; Secure; Path=/; Domain=sandbox.braintree-api.com; Expires=Tue, 24 Mar 2026 04:17:55 GMT",
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


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
