# Connector `worldpayvantiv` / Suite `authorize` / Scenario `no3ds_auto_capture_eps`

- Service: `PaymentService/Authorize`
- PM / PMT: `eps` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_eps_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_11189b4615dc4fbb92e8826d",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "eps": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "sam.8104@example.com"
    },
    "id": "cust_96945ce385bc452b9dc1f94c",
    "phone_number": "+912396634796"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "6874 Sunset Blvd"
      },
      "line2": {
        "value": "3520 Pine Ave"
      },
      "line3": {
        "value": "7574 Main Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "18548"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "jordan.3997@sandbox.example.com"
      },
      "phone_number": {
        "value": "1832498347"
      },
      "phone_country_code": "+43"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8608 Market St"
      },
      "line2": {
        "value": "7733 Pine Ln"
      },
      "line3": {
        "value": "628 Oak St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "26513"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "sam.8507@example.com"
      },
      "phone_number": {
        "value": "7692959786"
      },
      "phone_country_code": "+43"
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
  "description": "No3DS auto capture EPS payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_eps_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:16:50 GMT
x-request-id: authorize_no3ds_auto_capture_eps_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "BAD_REQUEST",
      "message": "Payment method is not supported by worldpayvantiv"
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
