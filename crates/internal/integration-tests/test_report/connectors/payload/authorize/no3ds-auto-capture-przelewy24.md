# Connector `payload` / Suite `authorize` / Scenario `no3ds_auto_capture_przelewy24`

- Service: `PaymentService/Authorize`
- PM / PMT: `przelewy24` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_przelewy24_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_przelewy24_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_5126ad22ff49457c9d46eb583768a20a",
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
    "name": "Liam Miller",
    "email": {
      "value": "morgan.1379@sandbox.example.com"
    },
    "id": "cust_b8ef8f55a42e44a9a638d0c43a3865ba",
    "phone_number": "+917010052070"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "1788 Sunset Dr"
      },
      "line2": {
        "value": "5026 Market Ln"
      },
      "line3": {
        "value": "8745 Pine Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "gen_574766"
      },
      "zip_code": {
        "value": "71609"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "alex.6141@testmail.io"
      },
      "phone_number": {
        "value": "5545949405"
      },
      "phone_country_code": "+48"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "7464 Main Dr"
      },
      "line2": {
        "value": "7620 Sunset St"
      },
      "line3": {
        "value": "5129 Market Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "gen_478056"
      },
      "zip_code": {
        "value": "92856"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "alex.7412@sandbox.example.com"
      },
      "phone_number": {
        "value": "8852695591"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_przelewy24_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_przelewy24_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:22:47 GMT
x-request-id: authorize_no3ds_auto_capture_przelewy24_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "BAD_REQUEST",
      "message": "Payment method is not supported by Payload"
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
