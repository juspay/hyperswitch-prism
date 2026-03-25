# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_51ae773a5eff4111bdf5f077",
  "amount": {
    "minor_amount": 6107,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "999"
      },
      "card_holder_name": {
        "value": "Ethan Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "jordan.7899@sandbox.example.com"
    },
    "id": "cust_81dfa5decb9f4058967f3cc3",
    "phone_number": "+14068419834"
  },
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
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "3089 Pine Ln"
      },
      "line2": {
        "value": "910 Market St"
      },
      "line3": {
        "value": "645 Lake Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91026"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1593@testmail.io"
      },
      "phone_number": {
        "value": "8758438627"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "8721 Market St"
      },
      "line2": {
        "value": "6349 Lake Blvd"
      },
      "line3": {
        "value": "801 Oak Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34502"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.9279@example.com"
      },
      "phone_number": {
        "value": "5146675592"
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
  "locale": "en-US"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:11:25 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_51ae773a5eff4111bdf5f077",
  "connectorTransactionId": "46054247",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Origin, Content-Type, X-Auth-Token, js-token, user-token, business-id",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Origin, Content-Type, X-Auth-Token, js-token",
    "access-control-max-age": "600",
    "alt-svc": "h3=\":443\"; ma=86400",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1384a99e6457ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:25 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "94",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=uNll.NO.RYRwARsrXkRKOBmYcaT3WaeLKlA0jCJtcYE-1774332683.7799633-1.0.1.1-inbNuaQggtbmGjNq3DacMf9PFVzzYzqP6UsDx.eipQTSyAioJhVU_4_jxdFUpXWJVW7H4FQlrd1IkMxnqQSdlZ9KRNERLjnT7IGcdO1Q2vHT40Nc6isn5I2AfxQ4XkrcH71DUnuFGp1bdr3g.INBSA; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:25 GMT",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked"
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
