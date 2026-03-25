# Connector `paybox` / Suite `authorize` / Scenario `no3ds_auto_capture_klarna`

- Service: `PaymentService/Authorize`
- PM / PMT: `klarna` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_klarna_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:04:57 GMT
x-request-id: authorize_no3ds_auto_capture_klarna_req
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
  -H "x-request-id: authorize_no3ds_auto_capture_klarna_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_1bfbc264a8e24951b0d39062",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "klarna": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Smith",
    "email": {
      "value": "morgan.6548@sandbox.example.com"
    },
    "id": "cust_4bdb8d52412e4bfc90e98a5d",
    "phone_number": "+15478730323"
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
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6571 Main Ln"
      },
      "line2": {
        "value": "1594 Lake Ln"
      },
      "line3": {
        "value": "6715 Main Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "94948"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4130@sandbox.example.com"
      },
      "phone_number": {
        "value": "9209078187"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "3179 Market Ave"
      },
      "line2": {
        "value": "9426 Lake Rd"
      },
      "line3": {
        "value": "1912 Lake Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44736"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7106@testmail.io"
      },
      "phone_number": {
        "value": "3744831415"
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
  "description": "No3DS auto capture Klarna payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_klarna_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:04:57 GMT
x-request-id: authorize_no3ds_auto_capture_klarna_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Failed to convert connector config from X-Connector-Config header
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
