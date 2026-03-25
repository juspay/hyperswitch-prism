# Connector `noon` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
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
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_bc5e5fd8cb824ee0bdf291be",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "999"
      },
      "card_holder_name": {
        "value": "Noah Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Taylor",
    "email": {
      "value": "riley.5703@testmail.io"
    },
    "id": "cust_2be79cf508b84552ae711302",
    "phone_number": "+15208678106"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "9784 Lake Blvd"
      },
      "line2": {
        "value": "6209 Lake Dr"
      },
      "line3": {
        "value": "3094 Lake Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "10795"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2953@sandbox.example.com"
      },
      "phone_number": {
        "value": "4237156822"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "5273 Oak Blvd"
      },
      "line2": {
        "value": "8576 Market Ln"
      },
      "line3": {
        "value": "3334 Pine Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "82310"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7278@testmail.io"
      },
      "phone_number": {
        "value": "1180265746"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
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
x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:45:28 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "19004",
      "message": "Field order category is not valid.",
      "reason": "Field order category is not valid."
    }
  },
  "statusCode": 403,
  "responseHeaders": {
    "akamai-cache-status": "Miss from child, Miss from parent",
    "alt-svc": "h3=\":443\"; ma=93600",
    "cache-control": "max-age=0",
    "connection": "close",
    "content-length": "270",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 01:45:28 GMT",
    "np-waf-trace-id": "0.0f0ec417.1774316728.5e7e0c7",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "referrer-policy": "no-referrer-when-downgrade",
    "server-timing": "ak_p; desc=\"1774316724556_398724623_99082439_35312_5886_461_440_-\";dur=1",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-apioperation": "INITIATE",
    "x-businessid": "hyperswitch",
    "x-classdescription": "Invalid BadRequest",
    "x-content-type-options": "nosniff",
    "x-merchantid": "hyperswitch",
    "x-message": "Field order category is not valid.",
    "x-resultcode": "19004"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
