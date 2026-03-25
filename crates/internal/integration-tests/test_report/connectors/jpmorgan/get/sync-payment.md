# Connector `jpmorgan` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"NOT_FOUND","message":"Transaction was not found","reason":"Transaction was not found"}}
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
  "merchant_transaction_id": "mti_ed346914e9ef46c5a0dc7ab7",
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
    "name": "Emma Johnson",
    "email": {
      "value": "casey.6949@sandbox.example.com"
    },
    "id": "cust_3f00034cc86243e3a96ee1bf",
    "phone_number": "+447685462764"
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
        "value": "Smith"
      },
      "line1": {
        "value": "1745 Sunset Ave"
      },
      "line2": {
        "value": "1590 Oak Dr"
      },
      "line3": {
        "value": "9889 Main Ln"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31909"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5533@sandbox.example.com"
      },
      "phone_number": {
        "value": "5063542016"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3312 Market Ln"
      },
      "line2": {
        "value": "7329 Main Ave"
      },
      "line3": {
        "value": "8010 Oak Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "28688"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.7131@example.com"
      },
      "phone_number": {
        "value": "5261053409"
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
date: Tue, 24 Mar 2026 05:47:17 GMT
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
  -H "x-request-id: get_sync_payment_req" \
  -H "x-connector-request-reference-id: get_sync_payment_ref" \
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
x-connector-request-reference-id: get_sync_payment_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:47:20 GMT
x-request-id: get_sync_payment_req

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
    "date": "Tue, 24 Mar 2026 05:47:20 GMT",
    "expires": "Tue, 24 Mar 2026 05:47:20 GMT",
    "pragma": "no-cache",
    "server-timing": "ak_p; desc=\"1774331238357_398553668_1101884441_233441_9336_8_0_-\";dur=1",
    "strict-transport-security": "max-age=86400 ; includeSubDomains",
    "x-jpmc-service-type": "sandbox"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJ0eXAiOiJKV1QiLCJraWQiOiJJR05rNSthbHVNdy9FeHQ4ejc5Wmg5ZVpZL0U9IiwiYWxnIjoiUlMyNTYifQ.eyJzdWIiOiJiOWNhMzc4NS03MzIzLTQwZTUtOTUzYS00OGM4MDc3YmFmMTciLCJjdHMiOiJPQVVUSDJfU1RBVEVMRVNTX0dSQU5UIiwiYXVkaXRUcmFja2luZ0lkIjoiZmFiNDkyOTktY2Q3YS00ZDQ3LWE4MTctZjM2Y2Y0MjFkNzU1LTQ3NzgxODQiLCJzdWJuYW1lIjoiYjljYTM3ODUtNzMyMy00MGU1LTk1M2EtNDhjODA3N2JhZjE3IiwiaXNzIjoiaHR0cHM6Ly9pZC5wYXltZW50cy5qcG1vcmdhbi5jb206NDQzL2FtL29hdXRoMiIsInRva2VuTmFtZSI6ImFjY2Vzc190b2tlbiIsInRva2VuX3R5cGUiOiJCZWFyZXIiLCJhdXRoR3JhbnRJZCI6IjhUMWp6eTl3eVh0S2RlWDNuVmFZOG5MNWUycyIsImNsaWVudF9pZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsImF1ZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsIm5iZiI6MTc3NDMzMTIzOCwiZ3JhbnRfdHlwZSI6ImNsaWVudF9jcmVkZW50aWFscyIsInNjb3BlIjpbImpwbTpwYXltZW50czpzYW5kYm94Il0sImF1dGhfdGltZSI6MTc3NDMzMTIzOCwicmVhbG0iOiIvYWxwaGEiLCJleHAiOjE3NzQzMzQ4MzgsImlhdCI6MTc3NDMzMTIzOCwiZXhwaXJlc19pbiI6MzYwMCwianRpIjoiTVhUbkRNQkJ1MkZnbHBpTDNNcUxmRmMzdHRVIn0.lsYrsYwgNwZY7xtLRt_5-WsYq2UBJx6h65D_X8vQ3R20s2vZysg-w2rdfn-QUyV2mzcXZlR2qNRwDsa6cN45Q3PAT-hye4HTLHCzAIrBarJcukZFK0aK5oQK80_6Nyolj_2QqM9m9Q20kNyNUYS2IOrSgAc5RMIHsFKjwI9O88dyoR21gYNa24aeagl7QP5u3mP6EQTf--xzZ5hN-DQ6gUPa6qVEvrXeO5mzrvYEOwKYpCxBjAMnCpaHDstzMdKaGo-aX6dua37lYQuGHbpKZsg0cf6QHJ1iYTFIuVsXYOIDwzOLF21KT321O0sQGtdsMR6Qe3m9YIgQYpgFR0fnWA"
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
