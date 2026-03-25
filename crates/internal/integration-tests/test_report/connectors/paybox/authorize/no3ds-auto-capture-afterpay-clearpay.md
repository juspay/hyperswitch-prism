# Connector `paybox` / Suite `authorize` / Scenario `no3ds_auto_capture_afterpay_clearpay`

- Service: `PaymentService/Authorize`
- PM / PMT: `afterpay_clearpay` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:04:56 GMT
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Failed to convert connector config from X-Connector-Config header
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_1ed42646a58349b78ce5140f",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "afterpay_clearpay": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Johnson",
    "email": {
      "value": "jordan.8887@sandbox.example.com"
    },
    "id": "cust_317c902643f246799d142763",
    "phone_number": "+916110661454"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1251 Main Rd"
      },
      "line2": {
        "value": "5355 Pine Dr"
      },
      "line3": {
        "value": "4109 Pine Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73739"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5435@sandbox.example.com"
      },
      "phone_number": {
        "value": "7825979958"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "1945 Market Dr"
      },
      "line2": {
        "value": "178 Main St"
      },
      "line3": {
        "value": "5781 Pine Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73800"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4813@testmail.io"
      },
      "phone_number": {
        "value": "9869480834"
      },
      "phone_country_code": "+1"
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
  "description": "No3DS auto capture Afterpay/Clearpay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:04:56 GMT
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Failed to convert connector config from X-Connector-Config header
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
