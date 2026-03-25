# Connector `redsys` / Suite `authorize` / Scenario `no3ds_auto_capture_bacs_bank_transfer`

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
  "merchant_transaction_id": "mti_4db591c2e0d1404780313715",
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
      "value": "morgan.3503@example.com"
    },
    "id": "cust_6b6b42c91dce4f06880ad703",
    "phone_number": "+13072077477"
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
        "value": "6049 Oak Rd"
      },
      "line2": {
        "value": "4196 Lake Blvd"
      },
      "line3": {
        "value": "1262 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "27819"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "jordan.4851@testmail.io"
      },
      "phone_number": {
        "value": "6722409384"
      },
      "phone_country_code": "+44"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8554 Market Ave"
      },
      "line2": {
        "value": "4134 Market St"
      },
      "line3": {
        "value": "8893 Market St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "35908"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "morgan.4143@testmail.io"
      },
      "phone_number": {
        "value": "7754702657"
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
date: Tue, 24 Mar 2026 07:09:46 GMT
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "BAD_REQUEST",
      "message": "Cards No3DS is not supported by Redsys"
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
