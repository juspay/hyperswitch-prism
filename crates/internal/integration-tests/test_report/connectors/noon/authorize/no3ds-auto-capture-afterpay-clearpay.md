# Connector `noon` / Suite `authorize` / Scenario `no3ds_auto_capture_afterpay_clearpay`

- Service: `PaymentService/Authorize`
- PM / PMT: `afterpay_clearpay` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_656465f4655745e4892e86de",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "afterpay_clearpay": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Brown",
    "email": {
      "value": "jordan.8358@sandbox.example.com"
    },
    "id": "cust_3f6dc7c04b9844999514a6ca",
    "phone_number": "+444631123105"
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
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8767 Pine St"
      },
      "line2": {
        "value": "6515 Main St"
      },
      "line3": {
        "value": "612 Sunset Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "22754"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3339@sandbox.example.com"
      },
      "phone_number": {
        "value": "6671981148"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "560 Oak Dr"
      },
      "line2": {
        "value": "916 Market Rd"
      },
      "line3": {
        "value": "4694 Pine Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "62661"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6776@testmail.io"
      },
      "phone_number": {
        "value": "5502946424"
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
  "description": "No3DS auto capture Afterpay/Clearpay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:45:10 GMT
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_IMPLEMENTED",
      "message": "This step has not been implemented for: Selected payment method through Noon"
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
