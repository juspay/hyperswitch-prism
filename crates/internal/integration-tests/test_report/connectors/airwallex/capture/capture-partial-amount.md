# Connector `airwallex` / Suite `capture` / Scenario `capture_partial_amount`

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
  "merchant_transaction_id": "mti_97f00c3c4db8481aa5b197d8",
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
        "value": "Ethan Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Smith",
    "email": {
      "value": "casey.6433@example.com"
    },
    "id": "cust_0b4bee03e6ec430389c986d0",
    "phone_number": "+19346687189"
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
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "8736 Sunset Dr"
      },
      "line2": {
        "value": "6888 Market Dr"
      },
      "line3": {
        "value": "2519 Oak Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "62336"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3028@sandbox.example.com"
      },
      "phone_number": {
        "value": "4550219204"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4891 Oak Blvd"
      },
      "line2": {
        "value": "425 Sunset Ln"
      },
      "line3": {
        "value": "9274 Oak Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "95797"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3529@example.com"
      },
      "phone_number": {
        "value": "1536610369"
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
date: Tue, 24 Mar 2026 05:15:46 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "BAD_REQUEST",
      "message": "Missing required field: merchant_order_id"
    }
  },
  "statusCode": 400
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
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_ba2f7515a97f450baeea8715",
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
date: Tue, 24 Mar 2026 05:15:47 GMT
x-request-id: capture_capture_partial_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "not_found",
      "message": "The requested endpoint does not exist [/api/v1/pa/payment_intents/auto_generate/capture]",
      "reason": "The requested endpoint does not exist [/api/v1/pa/payment_intents/auto_generate/capture]"
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "access-control-expose-headers": "Server-Timing,Server-Timing",
    "alt-svc": "h3=\":443\"; ma=2592000,h3-29=\":443\"; ma=2592000",
    "content-length": "180",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:15:46 GMT",
    "server": "APISIX",
    "server-timing": "traceparent;desc=\"00-4ab2c3271bcbdb5162aaf342bedd7cfd-35217231f99e3e2f-01\"",
    "strict-transport-security": "max-age=15552000",
    "via": "1.1 google, 1.1 google, 1.1 google",
    "x-b3-traceid": "4ab2c3271bcbdb5162aaf342bedd7cfd",
    "x-envoy-upstream-service-time": "17"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJraWQiOiJjNDRjODVkMDliMDc0NmNlYTIwZmI4NjZlYzI4YWY3ZSIsImFsZyI6IkhTMjU2In0.eyJ0eXBlIjoic2NvcGVkIiwiZGMiOiJISyIsImRhdGFfY2VudGVyX3JlZ2lvbiI6IkhLIiwiaXNzZGMiOiJVUyIsImFjY291bnRfaWQiOiIwODJhZDYyOC05ZWM4LTRmYTYtOTVlNi1mOGU1OGNmZTA5MTkiLCJhcGlfdmVyc2lvbiI6IjIwMjItMDItMTYiLCJwZXJtaXNzaW9ucyI6Ikg0c0lBQUFBQUFBQS80VlYwWTdqSUF6OGwzMWNuZklCL1pYVENUbkVhVkVKNUREWmJPL3J6N1RaRklocDN5TFBtTmg0UFB6K0NDZFl2MCs5c2RhNGMvZDUrdno0dGNWMDM0M0dnZE1HckFvNCt4Q3BCY2NBamtCSDQxM0I4UUZWRCs2YXpsN0JXb3hkRDVhVGtJcC9qZCtkOXU0TEE2VVREdEFTQXJyWUJZakh2QW5DRlNOL1ZQRy9pNi9aNXppNzlKL1JoQWxTcllvTGl2eFRrZVZRUnh3VVJXNUNaa1R1V0UzZzRJeFRxdS9BR2ZqV3lFUWhlekNCajFjRDlpSjh0cDd2U1lIV2ZuRVNnYWQxNWVMYWhQdEV4cnEzaTZmVTAyajlta1VOMFpJbUJFdTgrR0QrM2UrbVRQeWhhQWpEeGR1aFBqakhaU1NYU0VId3J2ZWNsVGlqc2RnRXcyalVsOEc4OEJrZTR6eFhzWVdITm5HSlpYZ3dOQzhSaS9NNXZLdTRJMWFEdlkreXlod1hONUNpMlpvME1vcWNVVXU5WWdWTVlnYmJwbFRJRExmMFl3VXN5R2x1b2R3dEhjdjdRWTJMYlRBcHBnRk55SE1mR2lBaHBhVXN3Y2R5bGJHQUxEZmsxVDZFVTlOWmNDZDJ0TXpKVmxpUmp4WFhwaG8vYWJDWUxhT285d2NwazdZRUcvZGw0cmIwNlNZRXltd2hqajVNU2w4Z25NVktkc3JtaDY4bzhnYlNqY3VjT3JtUkI3WlFuUlFEQWkzaGxwdk5acWZOdFZxeGgzbnVzZ1c2M3ZvM3VINkpGd3U0Q3UvRzJuNDNOSGNROGNsNS96Q3M3WWRoaHdxUFg5KzZkOG1RM1R2alZPNmRJNEo3WjdEczNtMkMwdFlUSG1teXlXZUVXbUxyRzY4KzRpVWlpRVVBdEF3a2VlQmdZb2J1L3R5eGxDd3E5cFJ1OEJPWXpUcWZyTUt4OTNEcDJIdjRwUjJMck5LT0pVcUZpSVo3UUhQRFBZQlB3ejFBdWFmdVlPYXBleXczenkyNDJjdjlyU3d1NW8xWDVpVEJLM080NFpVNXBlR1ZJcVgwU3BGU0N2blBmL1pZRlY2WENnQUEiLCJ0aWVyIjoiUyIsInJhdGVfbGltaXQiOmZhbHNlLCJzdWIiOiJlNTRhMWRiZC0wNzEwLTQ0M2MtYjM3Zi0zMmIzZDQzZjA2ZWYiLCJleHAiOjE3NzQzMzExNDcsImlhdCI6MTc3NDMyOTM0NywianRpIjoiOTQ5YzAxYTctYmRmYi00MjZmLTk0NTUtOWZhOTk4Y2U2YTlmIn0.egIgAIjpZJPal34OxdqFL6uWe8bhlu21-EpbvjDxsQM"
      },
      "expiresInSeconds": "1799",
      "tokenType": ***MASKED***"
    }
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
