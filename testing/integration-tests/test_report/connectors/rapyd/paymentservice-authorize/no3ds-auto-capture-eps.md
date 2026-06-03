# Connector `rapyd` / Suite `PaymentService/Authorize` / Scenario `EPS | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_eps`
- PM / PMT: `eps` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_eps_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_eps_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:16 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_eps_req
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
  -H "x-request-id: PaymentService/Authorize_no3ds_auto_capture_eps_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_eps_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_a30c78ffd7b241c6834d539e",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "eps": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "morgan.5922@example.com"
    },
    "id": "cust_993e2bfa55c44303ad580c07",
    "phone_number": "+916758943658",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2135 Lake Blvd"
      },
      "line2": {
        "value": "763 Oak Rd"
      },
      "line3": {
        "value": "9930 Market Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "44603"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "alex.3784@testmail.io"
      },
      "phone_number": {
        "value": "7149193857"
      },
      "phone_country_code": "+43"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "3142 Lake Blvd"
      },
      "line2": {
        "value": "7346 Oak Blvd"
      },
      "line3": {
        "value": "8899 Pine Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "36643"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "casey.9197@sandbox.example.com"
      },
      "phone_number": {
        "value": "5851824019"
      },
      "phone_country_code": "+43"
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
  "description": "No3DS auto capture EPS payment",
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_eps_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_eps_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:16 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_eps_req
Sent 1 request and received 0 responses

ERROR:
  Code: FailedPrecondition
  Message: This feature is not implemented: payment_method
```

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
