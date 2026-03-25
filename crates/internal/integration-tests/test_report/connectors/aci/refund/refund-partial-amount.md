# Connector `aci` / Suite `refund` / Scenario `refund_partial_amount`

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
  "merchant_transaction_id": "mti_277a25d4c1f54635ae2b820c",
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
        "value": "Ava Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "casey.2563@testmail.io"
    },
    "id": "cust_fb4b6886188b4de38d31fcd6",
    "phone_number": "+12243883691"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "3054 Sunset St"
      },
      "line2": {
        "value": "7626 Sunset Ave"
      },
      "line3": {
        "value": "8225 Lake Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "93487"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.4305@sandbox.example.com"
      },
      "phone_number": {
        "value": "1597008751"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9560 Pine Rd"
      },
      "line2": {
        "value": "6007 Oak Blvd"
      },
      "line3": {
        "value": "8222 Pine Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70799"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9346@sandbox.example.com"
      },
      "phone_number": {
        "value": "7053928949"
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
date: Mon, 23 Mar 2026 18:25:05 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "800.900.300",
      "message": "invalid authentication information"
    }
  },
  "statusCode": 401,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "close",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:25:05 GMT",
    "expires": "Mon, 23 Mar 2026 18:25:05 GMT",
    "pragma": "no-cache",
    "server": "ACI",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "tls-ciphers": "ECDHE-RSA-AES256-GCM-SHA384",
    "www-authenticate": "Bearer ***MASKED***, error=\"invalid_token\", error_description=\"Invalid Authorization header!\"",
    "x-application-waf-action": "allow",
    "x-content-type-options": "nosniff",
    "x-payon-ratepolicy": "auth-fail-opp",
    "x-xss-protection": "1; mode=block"
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
  "merchant_refund_id": "mri_e9d4db6642514b3d836f5e05",
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
date: Mon, 23 Mar 2026 18:25:06 GMT
x-request-id: refund_refund_partial_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "800.900.300",
      "message": "invalid authentication information"
    }
  },
  "statusCode": 401,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "close",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:25:06 GMT",
    "expires": "Mon, 23 Mar 2026 18:25:06 GMT",
    "pragma": "no-cache",
    "server": "ACI",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "tls-ciphers": "ECDHE-RSA-AES256-GCM-SHA384",
    "www-authenticate": "Bearer ***MASKED***, error=\"invalid_token\", error_description=\"Invalid Authorization header!\"",
    "x-application-waf-action": "allow",
    "x-content-type-options": "nosniff",
    "x-payon-ratepolicy": "auth-fail-opp",
    "x-xss-protection": "1; mode=block"
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


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
