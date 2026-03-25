# Connector `airwallex` / Suite `capture` / Scenario `capture_full_amount`

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
  "merchant_transaction_id": "mti_ade3dcc024bb463285d6c12f",
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
        "value": "Noah Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "riley.3377@sandbox.example.com"
    },
    "id": "cust_62c19653b9694734a579bb8c",
    "phone_number": "+916035062330"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "2013 Market St"
      },
      "line2": {
        "value": "3926 Sunset Rd"
      },
      "line3": {
        "value": "5133 Market Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97607"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8334@example.com"
      },
      "phone_number": {
        "value": "8905678790"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "6879 Market Blvd"
      },
      "line2": {
        "value": "5464 Market Dr"
      },
      "line3": {
        "value": "9792 Oak Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "94209"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1711@sandbox.example.com"
      },
      "phone_number": {
        "value": "2648174752"
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
date: Tue, 24 Mar 2026 05:15:45 GMT
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
  -H "x-request-id: capture_capture_full_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_32ed2b0917ab4036925ab53d",
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
x-connector-request-reference-id: capture_capture_full_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:15:46 GMT
x-request-id: capture_capture_full_amount_req

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
    "date": "Tue, 24 Mar 2026 05:15:45 GMT",
    "server": "APISIX",
    "server-timing": "traceparent;desc=\"00-97d543f0a675a61d567edab0cdbe81df-d7a9cad6b09a0a70-01\"",
    "strict-transport-security": "max-age=15552000",
    "via": "1.1 google, 1.1 google, 1.1 google",
    "x-b3-traceid": "97d543f0a675a61d567edab0cdbe81df",
    "x-envoy-upstream-service-time": "39"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJraWQiOiJjNDRjODVkMDliMDc0NmNlYTIwZmI4NjZlYzI4YWY3ZSIsImFsZyI6IkhTMjU2In0.eyJ0eXBlIjoic2NvcGVkIiwiZGMiOiJISyIsImRhdGFfY2VudGVyX3JlZ2lvbiI6IkhLIiwiaXNzZGMiOiJVUyIsImFjY291bnRfaWQiOiIwODJhZDYyOC05ZWM4LTRmYTYtOTVlNi1mOGU1OGNmZTA5MTkiLCJhcGlfdmVyc2lvbiI6IjIwMjItMDItMTYiLCJwZXJtaXNzaW9ucyI6Ikg0c0lBQUFBQUFBQS80VlYwWTdqSUF6OGwzMWNuZklCL1pYVENUbkVhVkVKNUREWmJPL3J6N1RaRklocDN5TFBtTmg0UFB6K0NDZFl2MCs5c2RhNGMvZDUrdno0dGNWMDM0M0dnZE1HckFvNCt4Q3BCY2NBamtCSDQxM0I4UUZWRCs2YXpsN0JXb3hkRDVhVGtJcC9qZCtkOXU0TEE2VVREdEFTQXJyWUJZakh2QW5DRlNOL1ZQRy9pNi9aNXppNzlKL1JoQWxTcllvTGl2eFRrZVZRUnh3VVJXNUNaa1R1V0UzZzRJeFRxdS9BR2ZqV3lFUWhlekNCajFjRDlpSjh0cDd2U1lIV2ZuRVNnYWQxNWVMYWhQdEV4cnEzaTZmVTAyajlta1VOMFpJbUJFdTgrR0QrM2UrbVRQeWhhQWpEeGR1aFBqakhaU1NYU0VId3J2ZWNsVGlqc2RnRXcyalVsOEc4OEJrZTR6eFhzWVdITm5HSlpYZ3dOQzhSaS9NNXZLdTRJMWFEdlkreXlod1hONUNpMlpvME1vcWNVVXU5WWdWTVlnYmJwbFRJRExmMFl3VXN5R2x1b2R3dEhjdjdRWTJMYlRBcHBnRk55SE1mR2lBaHBhVXN3Y2R5bGJHQUxEZmsxVDZFVTlOWmNDZDJ0TXpKVmxpUmp4WFhwaG8vYWJDWUxhT285d2NwazdZRUcvZGw0cmIwNlNZRXltd2hqajVNU2w4Z25NVktkc3JtaDY4bzhnYlNqY3VjT3JtUkI3WlFuUlFEQWkzaGxwdk5acWZOdFZxeGgzbnVzZ1c2M3ZvM3VINkpGd3U0Q3UvRzJuNDNOSGNROGNsNS96Q3M3WWRoaHdxUFg5KzZkOG1RM1R2alZPNmRJNEo3WjdEczNtMkMwdFlUSG1teXlXZUVXbUxyRzY4KzRpVWlpRVVBdEF3a2VlQmdZb2J1L3R5eGxDd3E5cFJ1OEJPWXpUcWZyTUt4OTNEcDJIdjRwUjJMck5LT0pVcUZpSVo3UUhQRFBZQlB3ejFBdWFmdVlPYXBleXczenkyNDJjdjlyU3d1NW8xWDVpVEJLM080NFpVNXBlR1ZJcVgwU3BGU0N2blBmL1pZRlY2WENnQUEiLCJ0aWVyIjoiUyIsInJhdGVfbGltaXQiOmZhbHNlLCJzdWIiOiJlNTRhMWRiZC0wNzEwLTQ0M2MtYjM3Zi0zMmIzZDQzZjA2ZWYiLCJleHAiOjE3NzQzMzExNDYsImlhdCI6MTc3NDMyOTM0NiwianRpIjoiYjFmNTFjNzQtMzc1ZC00ODAzLWFlNDUtYTBjMzRjYWQxMGUzIn0.tYn4W_rEjogaxJO5jkpGLGaVhbtyqWmXx36mKFvRKIM"
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
