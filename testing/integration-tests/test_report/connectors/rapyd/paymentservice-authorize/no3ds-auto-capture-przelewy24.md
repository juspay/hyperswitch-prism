# Connector `rapyd` / Suite `PaymentService/Authorize` / Scenario `Przelewy24 | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_przelewy24`
- PM / PMT: `przelewy24` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:17 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
Sent 1 request and received 0 responses

ERROR:
  Code: FailedPrecondition
  Message: This feature is not implemented: payment_method
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_01249f85178e426480aa14bd",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "przelewy24": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "jordan.4062@sandbox.example.com"
    },
    "id": "cust_a57573893b7d4b27bb5d8283",
    "phone_number": "+11601155612",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9593 Sunset Ave"
      },
      "line2": {
        "value": "3313 Main Rd"
      },
      "line3": {
        "value": "9171 Main Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "gen_166000"
      },
      "zip_code": {
        "value": "62321"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "casey.6522@testmail.io"
      },
      "phone_number": {
        "value": "7247274013"
      },
      "phone_country_code": "+48"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6747 Lake Ave"
      },
      "line2": {
        "value": "8843 Main Blvd"
      },
      "line3": {
        "value": "8131 Main Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "gen_325471"
      },
      "zip_code": {
        "value": "79323"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "jordan.8302@testmail.io"
      },
      "phone_number": {
        "value": "8155720810"
      },
      "phone_country_code": "+48"
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
  "description": "No3DS auto capture Przelewy24 payment",
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
  "order_details": []
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:17 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
Sent 1 request and received 0 responses

ERROR:
  Code: FailedPrecondition
  Message: This feature is not implemented: payment_method
```

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
