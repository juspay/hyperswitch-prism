# Connector `jpmorgan` / Suite `authorize` / Scenario `no3ds_auto_capture_bacs_bank_transfer`

- Service: `PaymentService/Authorize`
- PM / PMT: `bacs_bank_transfer` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bacs_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_5dd5b6199e0c47988289d741",
  "amount": {
    "minor_amount": 6000,
    "currency": "GBP"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "bacs_bank_transfer": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "jordan.7523@example.com"
    },
    "id": "cust_b2a664c1eff644f3b87608fd",
    "phone_number": "+449952484550"
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
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9792 Main Ln"
      },
      "line2": {
        "value": "4070 Sunset Ln"
      },
      "line3": {
        "value": "193 Pine Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "65921"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "morgan.5320@testmail.io"
      },
      "phone_number": {
        "value": "6264020218"
      },
      "phone_country_code": "+44"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "343 Oak Blvd"
      },
      "line2": {
        "value": "3200 Market Dr"
      },
      "line3": {
        "value": "3252 Sunset St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "77324"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "jordan.4192@sandbox.example.com"
      },
      "phone_number": {
        "value": "6194508015"
      },
      "phone_country_code": "+44"
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
  "description": "No3DS BACS bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bacs_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:47:00 GMT
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "INTERNAL_SERVER_ERROR",
      "message": "Failed to obtain authentication type"
    }
  },
  "statusCode": 500
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
