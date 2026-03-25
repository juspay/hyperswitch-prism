# Connector `airwallex` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
  "merchant_transaction_id": "mti_98022390ddef4988a45cc398",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "sam.6602@sandbox.example.com"
    },
    "id": "cust_4a157565fe36493bb9a075e1",
    "phone_number": "+18690101807"
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
        "value": "Brown"
      },
      "line1": {
        "value": "6946 Oak Ave"
      },
      "line2": {
        "value": "2680 Oak Blvd"
      },
      "line3": {
        "value": "3621 Main Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "37124"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9545@sandbox.example.com"
      },
      "phone_number": {
        "value": "6625491024"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "7654 Sunset Blvd"
      },
      "line2": {
        "value": "7288 Lake Ln"
      },
      "line3": {
        "value": "3366 Main Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31277"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8475@sandbox.example.com"
      },
      "phone_number": {
        "value": "9913837186"
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
date: Tue, 24 Mar 2026 05:15:53 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount": {
    "minor_amount": 6000,
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
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:15:54 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "not_found",
      "message": "The requested endpoint does not exist [/api/v1/pa/payment_intents/auto_generate]",
      "reason": "The requested endpoint does not exist [/api/v1/pa/payment_intents/auto_generate]"
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "access-control-expose-headers": "Server-Timing,Server-Timing",
    "alt-svc": "h3=\":443\"; ma=2592000,h3-29=\":443\"; ma=2592000",
    "content-length": "172",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:15:53 GMT",
    "server": "APISIX",
    "server-timing": "traceparent;desc=\"00-dd2baef31d2c5ec14eda9e5c81c280e4-ea1e4b577e5287e9-01\"",
    "strict-transport-security": "max-age=15552000",
    "via": "1.1 google, 1.1 google, 1.1 google",
    "x-b3-traceid": "dd2baef31d2c5ec14eda9e5c81c280e4",
    "x-envoy-upstream-service-time": "16"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJraWQiOiJjNDRjODVkMDliMDc0NmNlYTIwZmI4NjZlYzI4YWY3ZSIsImFsZyI6IkhTMjU2In0.eyJ0eXBlIjoic2NvcGVkIiwiZGMiOiJISyIsImRhdGFfY2VudGVyX3JlZ2lvbiI6IkhLIiwiaXNzZGMiOiJVUyIsImFjY291bnRfaWQiOiIwODJhZDYyOC05ZWM4LTRmYTYtOTVlNi1mOGU1OGNmZTA5MTkiLCJhcGlfdmVyc2lvbiI6IjIwMjItMDItMTYiLCJwZXJtaXNzaW9ucyI6Ikg0c0lBQUFBQUFBQS80VlYwWTdqSUF6OGwzMWNuZklCL1pYVENUbkVhVkVKNUREWmJPL3J6N1RaRklocDN5TFBtTmg0UFB6K0NDZFl2MCs5c2RhNGMvZDUrdno0dGNWMDM0M0dnZE1HckFvNCt4Q3BCY2NBamtCSDQxM0I4UUZWRCs2YXpsN0JXb3hkRDVhVGtJcC9qZCtkOXU0TEE2VVREdEFTQXJyWUJZakh2QW5DRlNOL1ZQRy9pNi9aNXppNzlKL1JoQWxTcllvTGl2eFRrZVZRUnh3VVJXNUNaa1R1V0UzZzRJeFRxdS9BR2ZqV3lFUWhlekNCajFjRDlpSjh0cDd2U1lIV2ZuRVNnYWQxNWVMYWhQdEV4cnEzaTZmVTAyajlta1VOMFpJbUJFdTgrR0QrM2UrbVRQeWhhQWpEeGR1aFBqakhaU1NYU0VId3J2ZWNsVGlqc2RnRXcyalVsOEc4OEJrZTR6eFhzWVdITm5HSlpYZ3dOQzhSaS9NNXZLdTRJMWFEdlkreXlod1hONUNpMlpvME1vcWNVVXU5WWdWTVlnYmJwbFRJRExmMFl3VXN5R2x1b2R3dEhjdjdRWTJMYlRBcHBnRk55SE1mR2lBaHBhVXN3Y2R5bGJHQUxEZmsxVDZFVTlOWmNDZDJ0TXpKVmxpUmp4WFhwaG8vYWJDWUxhT285d2NwazdZRUcvZGw0cmIwNlNZRXltd2hqajVNU2w4Z25NVktkc3JtaDY4bzhnYlNqY3VjT3JtUkI3WlFuUlFEQWkzaGxwdk5acWZOdFZxeGgzbnVzZ1c2M3ZvM3VINkpGd3U0Q3UvRzJuNDNOSGNROGNsNS96Q3M3WWRoaHdxUFg5KzZkOG1RM1R2alZPNmRJNEo3WjdEczNtMkMwdFlUSG1teXlXZUVXbUxyRzY4KzRpVWlpRVVBdEF3a2VlQmdZb2J1L3R5eGxDd3E5cFJ1OEJPWXpUcWZyTUt4OTNEcDJIdjRwUjJMck5LT0pVcUZpSVo3UUhQRFBZQlB3ejFBdWFmdVlPYXBleXczenkyNDJjdjlyU3d1NW8xWDVpVEJLM080NFpVNXBlR1ZJcVgwU3BGU0N2blBmL1pZRlY2WENnQUEiLCJ0aWVyIjoiUyIsInJhdGVfbGltaXQiOmZhbHNlLCJzdWIiOiJlNTRhMWRiZC0wNzEwLTQ0M2MtYjM3Zi0zMmIzZDQzZjA2ZWYiLCJleHAiOjE3NzQzMzExNTQsImlhdCI6MTc3NDMyOTM1NCwianRpIjoiOTRhMGI2ZjAtZTE3MS00YjU3LWEwNTktNDkxY2MyMTA1NmJkIn0.HzXJoP_fDfaV2RGtn46JVudguu7OAp2mCrEdt58Mces"
      },
      "expiresInSeconds": "1799",
      "tokenType": ***MASKED***"
    }
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


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
