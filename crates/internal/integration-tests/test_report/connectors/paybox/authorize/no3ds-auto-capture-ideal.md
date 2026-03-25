# Connector `paybox` / Suite `authorize` / Scenario `no3ds_auto_capture_ideal`

- Service: `PaymentService/Authorize`
- PM / PMT: `ideal` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_ideal_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:04:57 GMT
x-request-id: authorize_no3ds_auto_capture_ideal_req
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
  -H "x-request-id: authorize_no3ds_auto_capture_ideal_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_1ec56951d5544eeda0094da2",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "ideal": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "jordan.9926@sandbox.example.com"
    },
    "id": "cust_eb6bb7d3308241b7878119f0",
    "phone_number": "+448732448331"
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
        "value": "7294 Main Ave"
      },
      "line2": {
        "value": "9742 Sunset Ln"
      },
      "line3": {
        "value": "5419 Main Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70459"
      },
      "country_alpha2_code": "NL",
      "email": {
        "value": "riley.8364@example.com"
      },
      "phone_number": {
        "value": "5010046316"
      },
      "phone_country_code": "+31"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "113 Main Rd"
      },
      "line2": {
        "value": "448 Pine Ln"
      },
      "line3": {
        "value": "3275 Oak Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "75935"
      },
      "country_alpha2_code": "NL",
      "email": {
        "value": "alex.7464@testmail.io"
      },
      "phone_number": {
        "value": "9126337626"
      },
      "phone_country_code": "+31"
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
  "description": "No3DS auto capture iDEAL payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_ideal_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:04:57 GMT
x-request-id: authorize_no3ds_auto_capture_ideal_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Failed to convert connector config from X-Connector-Config header
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
