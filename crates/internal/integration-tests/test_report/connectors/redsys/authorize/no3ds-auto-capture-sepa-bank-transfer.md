# Connector `redsys` / Suite `authorize` / Scenario `no3ds_auto_capture_sepa_bank_transfer`

- Service: `PaymentService/Authorize`
- PM / PMT: `sepa_bank_transfer` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_sepa_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_sepa_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_501b6dab67704b71b0c5eaa5",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "sepa_bank_transfer": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "alex.7761@example.com"
    },
    "id": "cust_8ae5ad027efe4ced94d320f9",
    "phone_number": "+911444159247"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "8058 Pine Dr"
      },
      "line2": {
        "value": "1853 Lake Rd"
      },
      "line3": {
        "value": "1332 Market Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "BE"
      },
      "zip_code": {
        "value": "18777"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "sam.4911@sandbox.example.com"
      },
      "phone_number": {
        "value": "7349396122"
      },
      "phone_country_code": "+49"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1708 Oak St"
      },
      "line2": {
        "value": "2374 Oak St"
      },
      "line3": {
        "value": "8383 Main Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "BE"
      },
      "zip_code": {
        "value": "16345"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "alex.6848@testmail.io"
      },
      "phone_number": {
        "value": "2285692896"
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
  "description": "No3DS SEPA bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_sepa_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_sepa_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:09:49 GMT
x-request-id: authorize_no3ds_auto_capture_sepa_bank_transfer_req

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
