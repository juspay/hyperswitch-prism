# Connector `fiserv` / Suite `authorize` / Scenario `no3ds_auto_capture_giropay`

- Service: `PaymentService/Authorize`
- PM / PMT: `giropay` / `-`
- Result: `PASS`

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
  "merchant_transaction_id": "mti_b0c76ddfb78f423fa5756b61",
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
    "name": "Ethan Brown",
    "email": {
      "value": "alex.6559@example.com"
    },
    "id": "cust_cdac1252cf9645aa99e5ec63",
    "phone_number": "+918560544794"
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
        "value": "Miller"
      },
      "line1": {
        "value": "2489 Main Dr"
      },
      "line2": {
        "value": "247 Market St"
      },
      "line3": {
        "value": "3411 Main St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "39589"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "casey.7914@testmail.io"
      },
      "phone_number": {
        "value": "7029642728"
      },
      "phone_country_code": "+49"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6827 Main Rd"
      },
      "line2": {
        "value": "4834 Sunset Rd"
      },
      "line3": {
        "value": "2077 Oak Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "35351"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "casey.8622@example.com"
      },
      "phone_number": {
        "value": "2571454675"
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
date: Tue, 24 Mar 2026 07:41:40 GMT
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
      "message": "This step has not been implemented for: Selected payment method through fiserv"
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
