# Connector `cashtocode` / Suite `authorize` / Scenario `no3ds_auto_capture_przelewy24`

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
  "merchant_transaction_id": "mti_e804434811c34bb2b242f279",
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
    "name": "Noah Johnson",
    "email": {
      "value": "casey.3212@testmail.io"
    },
    "id": "cust_c6533cb9ce0840e69674bd74",
    "phone_number": "+449130907132"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "4248 Main St"
      },
      "line2": {
        "value": "3843 Lake Ave"
      },
      "line3": {
        "value": "3117 Pine Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "gen_461080"
      },
      "zip_code": {
        "value": "23418"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "sam.7656@testmail.io"
      },
      "phone_number": {
        "value": "1647580639"
      },
      "phone_country_code": "+48"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9091 Lake Dr"
      },
      "line2": {
        "value": "9673 Pine Dr"
      },
      "line3": {
        "value": "2917 Oak St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "gen_409424"
      },
      "zip_code": {
        "value": "18819"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "alex.1488@sandbox.example.com"
      },
      "phone_number": {
        "value": "1265328096"
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
date: Tue, 24 Mar 2026 06:59:41 GMT
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
      "message": "Payment Method Type not found"
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
