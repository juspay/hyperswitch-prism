# Connector `rapyd` / Suite `PaymentService/Authorize` / Scenario `Giropay | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_giropay`
- PM / PMT: `giropay` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_giropay_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_giropay_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:16 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_giropay_req
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
  -H "x-request-id: PaymentService/Authorize_no3ds_auto_capture_giropay_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_giropay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_25506478e0c141919fb75d7f",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "giropay": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Wilson",
    "email": {
      "value": "jordan.9076@sandbox.example.com"
    },
    "id": "cust_2850178836894bcb813bd5fe",
    "phone_number": "+443203697759",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3108 Oak Rd"
      },
      "line2": {
        "value": "3883 Oak Rd"
      },
      "line3": {
        "value": "726 Main St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "30604"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "riley.3554@sandbox.example.com"
      },
      "phone_number": {
        "value": "9921308318"
      },
      "phone_country_code": "+49"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "5171 Sunset Ln"
      },
      "line2": {
        "value": "3667 Market St"
      },
      "line3": {
        "value": "7728 Pine Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "53948"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "sam.6314@testmail.io"
      },
      "phone_number": {
        "value": "2938560026"
      },
      "phone_country_code": "+49"
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
  "description": "No3DS auto capture Giropay payment",
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_giropay_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_giropay_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:16 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_giropay_req
Sent 1 request and received 0 responses

ERROR:
  Code: FailedPrecondition
  Message: This feature is not implemented: payment_method
```

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
