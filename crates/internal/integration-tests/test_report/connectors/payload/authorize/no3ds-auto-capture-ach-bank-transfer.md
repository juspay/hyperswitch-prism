# Connector `payload` / Suite `authorize` / Scenario `no3ds_auto_capture_ach_bank_transfer`

- Service: `PaymentService/Authorize`
- PM / PMT: `ach_bank_transfer` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_ach_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_ach_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_5c296f8cc47540739cf45e31fbd229ee",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "ach_bank_transfer": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "casey.7716@testmail.io"
    },
    "id": "cust_f94686099aca4d48addd8b041064010b",
    "phone_number": "+916522570771"
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
        "value": "Miller"
      },
      "line1": {
        "value": "5581 Main Ln"
      },
      "line2": {
        "value": "5410 Pine Blvd"
      },
      "line3": {
        "value": "9212 Oak Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "29103"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7342@testmail.io"
      },
      "phone_number": {
        "value": "5997772428"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3327 Sunset Ave"
      },
      "line2": {
        "value": "9139 Lake Ave"
      },
      "line3": {
        "value": "4043 Market Ln"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "15527"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1350@example.com"
      },
      "phone_number": {
        "value": "7396658852"
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
  "description": "No3DS ACH bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_ach_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_ach_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:22:41 GMT
x-request-id: authorize_no3ds_auto_capture_ach_bank_transfer_req

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
