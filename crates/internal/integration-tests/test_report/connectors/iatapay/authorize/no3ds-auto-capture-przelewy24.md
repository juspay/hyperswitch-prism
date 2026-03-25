# Connector `iatapay` / Suite `authorize` / Scenario `no3ds_auto_capture_przelewy24`

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
  "merchant_transaction_id": "mti_6d6757c050de4104a3456fe4",
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
    "name": "Ava Taylor",
    "email": {
      "value": "casey.9194@testmail.io"
    },
    "id": "cust_f23cb1d58a6f453bbe31e4bc",
    "phone_number": "+445083356640"
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
        "value": "4268 Pine Ave"
      },
      "line2": {
        "value": "1800 Oak St"
      },
      "line3": {
        "value": "8627 Market Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "gen_929666"
      },
      "zip_code": {
        "value": "19099"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "jordan.8382@testmail.io"
      },
      "phone_number": {
        "value": "5695959374"
      },
      "phone_country_code": "+48"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "5807 Sunset Dr"
      },
      "line2": {
        "value": "5145 Lake Rd"
      },
      "line3": {
        "value": "2825 Oak St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "gen_776815"
      },
      "zip_code": {
        "value": "48518"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "casey.4816@example.com"
      },
      "phone_number": {
        "value": "6729984650"
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
date: Tue, 24 Mar 2026 07:02:53 GMT
x-request-id: authorize_no3ds_auto_capture_przelewy24_req

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
