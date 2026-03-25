# Connector `jpmorgan` / Suite `authorize` / Scenario `no3ds_auto_capture_eps`

- Service: `PaymentService/Authorize`
- PM / PMT: `eps` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_eps_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_9346f798b22a4cf9919d3ac2",
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
    "name": "Noah Johnson",
    "email": {
      "value": "riley.8323@testmail.io"
    },
    "id": "cust_e2b156cc4a0d449b8223189b",
    "phone_number": "+443728567185"
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
        "value": "Brown"
      },
      "line1": {
        "value": "3166 Oak Ln"
      },
      "line2": {
        "value": "2326 Oak Blvd"
      },
      "line3": {
        "value": "6770 Main Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "51618"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "morgan.7692@example.com"
      },
      "phone_number": {
        "value": "2636215381"
      },
      "phone_country_code": "+43"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "7754 Sunset Rd"
      },
      "line2": {
        "value": "6138 Main Dr"
      },
      "line3": {
        "value": "9308 Main Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "15814"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "jordan.4010@testmail.io"
      },
      "phone_number": {
        "value": "4048033274"
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
date: Tue, 24 Mar 2026 05:47:01 GMT
x-request-id: authorize_no3ds_auto_capture_eps_req

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
