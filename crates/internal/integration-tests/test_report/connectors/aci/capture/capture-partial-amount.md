# Connector `aci` / Suite `capture` / Scenario `capture_partial_amount`

- Service: `PaymentService/Capture`
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
  "merchant_transaction_id": "mti_12ea8262e37444af9e900efa",
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
        "value": "Ava Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "riley.3664@testmail.io"
    },
    "id": "cust_55b7dcc061f541969586175e",
    "phone_number": "+449045317091"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "6082 Lake St"
      },
      "line2": {
        "value": "8701 Oak Dr"
      },
      "line3": {
        "value": "7855 Pine Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "19240"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.7015@example.com"
      },
      "phone_number": {
        "value": "7999881387"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "1026 Main Dr"
      },
      "line2": {
        "value": "9511 Market Ln"
      },
      "line3": {
        "value": "5986 Oak Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42605"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8234@sandbox.example.com"
      },
      "phone_number": {
        "value": "3625133665"
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
date: Mon, 23 Mar 2026 18:24:59 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

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
    "date": "Mon, 23 Mar 2026 18:24:59 GMT",
    "expires": "Mon, 23 Mar 2026 18:24:59 GMT",
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
  -H "x-request-id: capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_dbe45e359028495f90d800bf",
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
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:24:59 GMT
x-request-id: capture_capture_partial_amount_req

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
    "date": "Mon, 23 Mar 2026 18:24:59 GMT",
    "expires": "Mon, 23 Mar 2026 18:24:59 GMT",
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
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
