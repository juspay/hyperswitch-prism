# Connector `payload` / Suite `authorize` / Scenario `no3ds_auto_capture_eps`

- Service: `PaymentService/Authorize`
- PM / PMT: `eps` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_eps_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_b376c84add3c4ad3b5b2365b7d3ced03",
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
    "name": "Liam Brown",
    "email": {
      "value": "morgan.5415@example.com"
    },
    "id": "cust_8f8ace8790d54cc5bf4f48cd3dfa0653",
    "phone_number": "+913371546576"
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
        "value": "Miller"
      },
      "line1": {
        "value": "2330 Oak Dr"
      },
      "line2": {
        "value": "4682 Oak St"
      },
      "line3": {
        "value": "7839 Oak Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "80324"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "sam.4672@sandbox.example.com"
      },
      "phone_number": {
        "value": "3537751706"
      },
      "phone_country_code": "+43"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "197 Lake Dr"
      },
      "line2": {
        "value": "9937 Lake Ln"
      },
      "line3": {
        "value": "1302 Oak St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "15838"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "riley.4643@example.com"
      },
      "phone_number": {
        "value": "3150679734"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_eps_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:22:46 GMT
x-request-id: authorize_no3ds_auto_capture_eps_req

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
