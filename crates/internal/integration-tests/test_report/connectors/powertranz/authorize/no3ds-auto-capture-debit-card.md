# Connector `powertranz` / Suite `authorize` / Scenario `no3ds_auto_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
- Result: `PASS`

**Pre Requisites Executed**

- None
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
  "merchant_transaction_id": "mti_081070aa8ae644a68df0770b",
  "amount": {
    "minor_amount": 6000,
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
        "value": "Ava Miller"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "alex.8170@sandbox.example.com"
    },
    "id": "cust_6eea4b8625fd4ab480b0cb62",
    "phone_number": "+13440062378"
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
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "4333 Oak Ave"
      },
      "line2": {
        "value": "2781 Market Ln"
      },
      "line3": {
        "value": "2307 Main Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97805"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7290@testmail.io"
      },
      "phone_number": {
        "value": "1038793828"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "9358 Market Blvd"
      },
      "line2": {
        "value": "641 Market Rd"
      },
      "line3": {
        "value": "357 Oak Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12153"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6425@testmail.io"
      },
      "phone_number": {
        "value": "9284497169"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:06:18 GMT
x-request-id: authorize_no3ds_auto_capture_debit_card_req

Response contents:
{
  "connectorTransactionId": "ea6ee4d4-f433-4b79-95f0-7cd1b589f88a",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d5139e4047e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:18 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=96eitdgaemVFC3MhB7L.Y7PM9bjOEYCfoxly9NaQsGs-1774335977.5324264-1.0.1.1-U5ScbAA5ozERujUTTieVJfzqQLeV1PvRcSk3gVseUEfnpSdeDGCP_gIRy6KjqTZmliRwIg4bBqZGLCjSWC.7zXVFvhcAaxqFv3xM9YbuT8un6_kPoRlSabNPR4u9a5PJ; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:18 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
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
