# Connector `worldpayxml` / Suite `authorize` / Scenario `no3ds_auto_capture_klarna`

- Service: `PaymentService/Authorize`
- PM / PMT: `klarna` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_klarna_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_2a2f35a6122a4740b89c64dc",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "klarna": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Johnson",
    "email": {
      "value": "morgan.5767@example.com"
    },
    "id": "cust_5b61dedf2cb347b881d6a4a2",
    "phone_number": "+447563899556"
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
        "value": "9243 Main Ave"
      },
      "line2": {
        "value": "9542 Pine Blvd"
      },
      "line3": {
        "value": "7576 Pine Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "23769"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1727@sandbox.example.com"
      },
      "phone_number": {
        "value": "5770784766"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "7183 Pine Rd"
      },
      "line2": {
        "value": "6804 Market Ave"
      },
      "line3": {
        "value": "9270 Sunset Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "28698"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8169@sandbox.example.com"
      },
      "phone_number": {
        "value": "5347310596"
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
  "description": "No3DS auto capture Klarna payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_klarna_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:11:19 GMT
x-request-id: authorize_no3ds_auto_capture_klarna_req

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
