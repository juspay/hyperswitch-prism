# Connector `noon` / Suite `refund` / Scenario `refund_partial_amount`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

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
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_f5060e905bfe4c94a298cb67",
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
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "riley.6182@sandbox.example.com"
    },
    "id": "cust_d052250129a647988209ac25",
    "phone_number": "+441493708244"
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
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4205 Main St"
      },
      "line2": {
        "value": "9909 Sunset Ave"
      },
      "line3": {
        "value": "9858 Sunset Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83670"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.3574@sandbox.example.com"
      },
      "phone_number": {
        "value": "7006348937"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "5496 Pine St"
      },
      "line2": {
        "value": "3137 Lake Dr"
      },
      "line3": {
        "value": "4544 Pine Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34974"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1946@sandbox.example.com"
      },
      "phone_number": {
        "value": "9797222189"
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
  "description": "No3DS auto capture card payment (credit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:45:54 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

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
    "date": "Tue, 24 Mar 2026 01:45:52 GMT",
    "np-waf-trace-id": "0.370ec417.1774316752.cfdec0a",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "referrer-policy": "no-referrer-when-downgrade",
    "server-timing": "ak_p; desc=\"1774316751189_398724663_217967626_33671_5821_436_440_-\";dur=1",
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
  -H "x-request-id: refund_refund_partial_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_b4ae0dd0656a4a9581748a1d",
  "connector_transaction_id": "auto_generate",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 3000,
    "currency": "USD"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:45:56 GMT
x-request-id: refund_refund_partial_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "5036",
      "message": "Member 'order.id' has invalid value. Line 1, position 54.",
      "reason": "Member 'order.id' has invalid value. Line 1, position 54."
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
    "date": "Tue, 24 Mar 2026 01:45:56 GMT",
    "np-waf-trace-id": "0.370ec417.1774316756.cfded4b",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "referrer-policy": "no-referrer-when-downgrade",
    "server-timing": "ak_p; desc=\"1774316755166_398724663_217967947_34696_5913_438_442_-\";dur=1",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-apioperation": "REFUND",
    "x-classdescription": "Invalid BadRequest",
    "x-content-type-options": "nosniff",
    "x-merchantid": "hyperswitch",
    "x-message": "Member 'order.id' has invalid value. Line 1, position 54.",
    "x-resultcode": "5036"
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
