# Connector `worldpayxml` / Suite `authorize` / Scenario `no3ds_auto_capture_giropay`

- Service: `PaymentService/Authorize`
- PM / PMT: `giropay` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_giropay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_giropay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_dc24981a5a964c41825ec006",
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
    "name": "Ethan Smith",
    "email": {
      "value": "riley.1200@testmail.io"
    },
    "id": "cust_cdcea59260ab4f169b437a08",
    "phone_number": "+913434869382"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "6719 Market Ln"
      },
      "line2": {
        "value": "8257 Lake Dr"
      },
      "line3": {
        "value": "6381 Pine Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "42583"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "sam.9316@sandbox.example.com"
      },
      "phone_number": {
        "value": "2644403093"
      },
      "phone_country_code": "+49"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "8282 Market Ave"
      },
      "line2": {
        "value": "6522 Lake Ave"
      },
      "line3": {
        "value": "2048 Lake Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "44119"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "riley.4917@sandbox.example.com"
      },
      "phone_number": {
        "value": "6160372830"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_giropay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_giropay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:11:18 GMT
x-request-id: authorize_no3ds_auto_capture_giropay_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "BAD_REQUEST",
      "message": "Selected payment method is not supported by worldpayxml"
    }
  },
  "statusCode": 400
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
