# Connector `noon` / Suite `void` / Scenario `void_authorized_payment`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_manual_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_0bb210231ebb49fa88a39098",
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
        "value": "Ava Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "alex.1194@example.com"
    },
    "id": "cust_38ba9ea6516b4a82b583ae3c",
    "phone_number": "+917687561616"
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
        "value": "7088 Sunset St"
      },
      "line2": {
        "value": "7138 Lake Ave"
      },
      "line3": {
        "value": "6862 Market Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "40163"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4253@sandbox.example.com"
      },
      "phone_number": {
        "value": "8952399259"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "9230 Lake Rd"
      },
      "line2": {
        "value": "2177 Sunset Rd"
      },
      "line3": {
        "value": "7343 Sunset Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21996"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2448@testmail.io"
      },
      "phone_number": {
        "value": "7536741947"
      },
      "phone_country_code": "+91"
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
  "description": "No3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:45:31 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

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
    "date": "Tue, 24 Mar 2026 01:45:31 GMT",
    "np-waf-trace-id": "0.0f0ec417.1774316730.5e7e1e0",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "referrer-policy": "no-referrer-when-downgrade",
    "server-timing": "ak_p; desc=\"1774316729849_398724623_99082720_29010_5961_418_438_-\";dur=1",
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: void_void_authorized_payment_req" \
  -H "x-connector-request-reference-id: void_void_authorized_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "merchant_void_id": "mvi_bf6fb851bead4c7c9913d73e",
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
  "cancellation_reason": "requested_by_customer"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Cancel an authorized payment before capture. Releases held funds back to
// customer, typically used when orders are cancelled or abandoned.
rpc Void ( .types.PaymentServiceVoidRequest ) returns ( .types.PaymentServiceVoidResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: void_void_authorized_payment_ref
x-merchant-id: test_merchant
x-request-id: void_void_authorized_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:45:33 GMT
x-request-id: void_void_authorized_payment_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "5036",
      "message": "Member 'order.id' has invalid value. Line 1, position 55.",
      "reason": "Member 'order.id' has invalid value. Line 1, position 55."
    }
  },
  "statusCode": 403,
  "responseHeaders": {
    "akamai-cache-status": "Miss from child, Miss from parent",
    "alt-svc": "h3=\":443\"; ma=93600",
    "cache-control": "max-age=0",
    "connection": "close",
    "content-length": "241",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 01:45:33 GMT",
    "np-waf-trace-id": "0.0f0ec417.1774316732.5e7e245",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "referrer-policy": "no-referrer-when-downgrade",
    "server-timing": "ak_p; desc=\"1774316732054_398724623_99082821_37495_5997_438_444_-\";dur=1",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-apioperation": "REVERSE",
    "x-classdescription": "Invalid BadRequest",
    "x-content-type-options": "nosniff",
    "x-merchantid": "hyperswitch",
    "x-message": "Member 'order.id' has invalid value. Line 1, position 55.",
    "x-resultcode": "5036"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
