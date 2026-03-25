# Connector `adyen` / Suite `authorize` / Scenario `no3ds_auto_capture_bacs_bank_transfer`

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
  "merchant_transaction_id": "mti_e0e6dd234a85437bab67fbe7",
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
    "name": "Emma Johnson",
    "email": {
      "value": "jordan.5283@example.com"
    },
    "id": "cust_ff8fe7d1f9094314961e1c64",
    "phone_number": "+13406382434"
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
        "value": "8834 Oak Blvd"
      },
      "line2": {
        "value": "1335 Lake Dr"
      },
      "line3": {
        "value": "2113 Market Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "99742"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "sam.1757@example.com"
      },
      "phone_number": {
        "value": "7636991778"
      },
      "phone_country_code": "+44"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4324 Market St"
      },
      "line2": {
        "value": "9501 Sunset Blvd"
      },
      "line3": {
        "value": "2424 Sunset Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "27141"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "alex.4149@example.com"
      },
      "phone_number": {
        "value": "4551570750"
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
date: Tue, 24 Mar 2026 03:26:09 GMT
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_IMPLEMENTED",
      "message": "This step has not been implemented for: Selected payment method through Adyen"
    }
  },
  "statusCode": 501
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
