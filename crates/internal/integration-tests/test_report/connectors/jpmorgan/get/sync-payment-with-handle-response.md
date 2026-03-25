# Connector `jpmorgan` / Suite `get` / Scenario `sync_payment_with_handle_response`

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
  "merchant_transaction_id": "mti_ab0e20c0a64c400b98f802e0",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "jordan.1946@example.com"
    },
    "id": "cust_6cfe108fa11e4f27b88c9389",
    "phone_number": "+919360727471"
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
        "value": "8571 Market Blvd"
      },
      "line2": {
        "value": "3502 Market Blvd"
      },
      "line3": {
        "value": "6848 Sunset St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44523"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7807@example.com"
      },
      "phone_number": {
        "value": "6733034406"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5867 Sunset Dr"
      },
      "line2": {
        "value": "8582 Lake Blvd"
      },
      "line3": {
        "value": "2748 Sunset Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "92785"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.8946@testmail.io"
      },
      "phone_number": {
        "value": "4035402719"
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
date: Tue, 24 Mar 2026 05:47:21 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

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
date: Tue, 24 Mar 2026 05:47:24 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_FOUND",
      "message": "Transaction was not found",
      "reason": "Transaction was not found"
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "112",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 05:47:24 GMT",
    "expires": "Tue, 24 Mar 2026 05:47:24 GMT",
    "pragma": "no-cache",
    "server-timing": "ak_p; desc=\"1774331241567_398553668_1101929262_267734_9311_8_0_-\";dur=1",
    "strict-transport-security": "max-age=86400 ; includeSubDomains",
    "x-jpmc-service-type": "sandbox"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJ0eXAiOiJKV1QiLCJraWQiOiJJR05rNSthbHVNdy9FeHQ4ejc5Wmg5ZVpZL0U9IiwiYWxnIjoiUlMyNTYifQ.eyJzdWIiOiJiOWNhMzc4NS03MzIzLTQwZTUtOTUzYS00OGM4MDc3YmFmMTciLCJjdHMiOiJPQVVUSDJfU1RBVEVMRVNTX0dSQU5UIiwiYXVkaXRUcmFja2luZ0lkIjoiZTAzMmRkMTAtZTY4Yi00OTk2LWE4MzktMmRkYWRjNjQ3YmU3LTQ3OTAxODQiLCJzdWJuYW1lIjoiYjljYTM3ODUtNzMyMy00MGU1LTk1M2EtNDhjODA3N2JhZjE3IiwiaXNzIjoiaHR0cHM6Ly9pZC5wYXltZW50cy5qcG1vcmdhbi5jb206NDQzL2FtL29hdXRoMiIsInRva2VuTmFtZSI6ImFjY2Vzc190b2tlbiIsInRva2VuX3R5cGUiOiJCZWFyZXIiLCJhdXRoR3JhbnRJZCI6Ikc5OTdfZTBpQ3ZQR29rV2ljS1UwMzUtcUFrSSIsImNsaWVudF9pZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsImF1ZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsIm5iZiI6MTc3NDMzMTI0MSwiZ3JhbnRfdHlwZSI6ImNsaWVudF9jcmVkZW50aWFscyIsInNjb3BlIjpbImpwbTpwYXltZW50czpzYW5kYm94Il0sImF1dGhfdGltZSI6MTc3NDMzMTI0MSwicmVhbG0iOiIvYWxwaGEiLCJleHAiOjE3NzQzMzQ4NDEsImlhdCI6MTc3NDMzMTI0MSwiZXhwaXJlc19pbiI6MzYwMCwianRpIjoiYU9VNnhJUWRNNDUxVDA4VF9ELVgtZWdJdGtNIn0.GyEnEmd9YVH_exAj2S8rnxBlXI1F8ufPsLu8unI5Gd4UfmSjtd_k1QZw69HBR9V-S8BqjppO0zMX_jAXWRzK11NAC3uqNozn47Ou0sLkmWsLh4bQ18hhp9ixApUhpNAyRMRqyZ6WlVLEVKTmt0onhGKvxGXyTAidVUV9ztRrsnv2bDu8YzrA1D_xBHmNvNO7G5JrnJx7A3xuQn0C9aT4r9OsYQ9Hysc-gHWjqEJuFnsgGtNLdkum8fCNOwl4LiQBfIv1igWnqxuu8aapV8zB08e9lRROvU7_GgEqjYLKl2b5rzfC0rXcSC7kxvrlc7XtBxjdYfei8AJLykr8CJdNzw"
      },
      "expiresInSeconds": "3599",
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
