# Connector `jpmorgan` / Suite `authorize` / Scenario `no3ds_auto_capture_bancontact`

- Service: `PaymentService/Authorize`
- PM / PMT: `bancontact_card` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_bancontact_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_2674a4d3afa54fac9426ea6f",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "bancontact_card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_holder_name": {
        "value": "Emma Taylor"
      }
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.1163@sandbox.example.com"
    },
    "id": "cust_26e2c8d4ec87490a91a32f20",
    "phone_number": "+447383252173"
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
        "value": "Smith"
      },
      "line1": {
        "value": "3741 Sunset St"
      },
      "line2": {
        "value": "4446 Main Dr"
      },
      "line3": {
        "value": "2238 Sunset Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "44281"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "sam.8798@sandbox.example.com"
      },
      "phone_number": {
        "value": "8217244532"
      },
      "phone_country_code": "+32"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2165 Pine Ln"
      },
      "line2": {
        "value": "4113 Oak Blvd"
      },
      "line3": {
        "value": "1817 Market Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "27891"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "jordan.2386@example.com"
      },
      "phone_number": {
        "value": "9905425254"
      },
      "phone_country_code": "+32"
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
  "description": "No3DS auto capture Bancontact payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bancontact_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:47:00 GMT
x-request-id: authorize_no3ds_auto_capture_bancontact_req

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
