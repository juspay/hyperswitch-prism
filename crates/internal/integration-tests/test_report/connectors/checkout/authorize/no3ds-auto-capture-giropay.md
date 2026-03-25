# Connector `checkout` / Suite `authorize` / Scenario `no3ds_auto_capture_giropay`

- Service: `PaymentService/Authorize`
- PM / PMT: `giropay` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_giropay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_giropay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_131ffe5e2ec24109ae628656",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "giropay": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "jordan.6580@testmail.io"
    },
    "id": "cust_1f614d4142f4411fba959de7",
    "phone_number": "+912105921697"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "7270 Main Rd"
      },
      "line2": {
        "value": "1752 Oak Ave"
      },
      "line3": {
        "value": "8570 Main St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "51630"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "alex.6070@testmail.io"
      },
      "phone_number": {
        "value": "4865223509"
      },
      "phone_country_code": "+49"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4933 Market Blvd"
      },
      "line2": {
        "value": "7039 Main Ave"
      },
      "line3": {
        "value": "3607 Lake Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "70033"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "jordan.3821@testmail.io"
      },
      "phone_number": {
        "value": "4793637761"
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
  "description": "No3DS auto capture Giropay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_giropay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_giropay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:39:06 GMT
x-request-id: authorize_no3ds_auto_capture_giropay_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_IMPLEMENTED",
      "message": "This step has not been implemented for: Selected payment method through checkout"
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
