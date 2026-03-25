# Connector `jpmorgan` / Suite `capture` / Scenario `capture_full_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `PASS`

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
  "merchant_transaction_id": "mti_a3d6a1a4e7ef40fe9083a37f",
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
        "value": "Ethan Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Taylor",
    "email": {
      "value": "sam.8188@sandbox.example.com"
    },
    "id": "cust_f9f285271e3f44a3a27e34d8",
    "phone_number": "+447770138162"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "2609 Main Dr"
      },
      "line2": {
        "value": "4386 Pine St"
      },
      "line3": {
        "value": "7321 Market Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99958"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6259@testmail.io"
      },
      "phone_number": {
        "value": "8293436103"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1959 Oak Rd"
      },
      "line2": {
        "value": "4037 Pine Dr"
      },
      "line3": {
        "value": "4303 Main Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "94628"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1545@example.com"
      },
      "phone_number": {
        "value": "9974396505"
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
date: Tue, 24 Mar 2026 05:47:04 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

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
  "merchant_capture_id": "mci_c61f769d076f402791f99e6e",
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
date: Tue, 24 Mar 2026 05:47:09 GMT
x-request-id: capture_capture_full_amount_req

Response contents:
{
  "connectorTransactionId": "auto_generate",
  "status": "CHARGED",
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 05:47:09 GMT",
    "expires": "Tue, 24 Mar 2026 05:47:09 GMT",
    "pragma": "no-cache",
    "server-timing": "ak_p; desc=\"1774331225347_398553668_1101764053_424287_6227_8_41_-\";dur=1",
    "strict-transport-security": "max-age=86400 ; includeSubDomains",
    "vary": "Accept-Encoding",
    "x-jpmc-service-type": "sandbox"
  },
  "merchantCaptureId": "mci_c61f769d076f402791f99e6e",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJ0eXAiOiJKV1QiLCJraWQiOiJJR05rNSthbHVNdy9FeHQ4ejc5Wmg5ZVpZL0U9IiwiYWxnIjoiUlMyNTYifQ.eyJzdWIiOiJiOWNhMzc4NS03MzIzLTQwZTUtOTUzYS00OGM4MDc3YmFmMTciLCJjdHMiOiJPQVVUSDJfU1RBVEVMRVNTX0dSQU5UIiwiYXVkaXRUcmFja2luZ0lkIjoiZGNmYTBhNDQtMTQ3ZS00OGQ0LWExMWYtOTNmMjc4MmVlM2NiLTQ3OTQ1NzAiLCJzdWJuYW1lIjoiYjljYTM3ODUtNzMyMy00MGU1LTk1M2EtNDhjODA3N2JhZjE3IiwiaXNzIjoiaHR0cHM6Ly9pZC5wYXltZW50cy5qcG1vcmdhbi5jb206NDQzL2FtL29hdXRoMiIsInRva2VuTmFtZSI6ImFjY2Vzc190b2tlbiIsInRva2VuX3R5cGUiOiJCZWFyZXIiLCJhdXRoR3JhbnRJZCI6IklROUh3RVdSenAwdmNOeG4tOHVSQmtYdFlxdyIsImNsaWVudF9pZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsImF1ZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsIm5iZiI6MTc3NDMzMTIyNCwiZ3JhbnRfdHlwZSI6ImNsaWVudF9jcmVkZW50aWFscyIsInNjb3BlIjpbImpwbTpwYXltZW50czpzYW5kYm94Il0sImF1dGhfdGltZSI6MTc3NDMzMTIyNCwicmVhbG0iOiIvYWxwaGEiLCJleHAiOjE3NzQzMzQ4MjQsImlhdCI6MTc3NDMzMTIyNCwiZXhwaXJlc19pbiI6MzYwMCwianRpIjoicmVQckoxUGFiQkxTRHJJbVN2NVFNekVmazQ0In0.UQxBYv8JAmxrMVFuMum-zX426k26bgsyIu839-VaLfAnPRwhit30rhqP6_uhkbhY7plA60VzO71J1_n88OjS2bMGIY8n0PDuy-d6nLK7H3FU7sDxZoyiKA9dHFp9HTcyTKeMsC9wcQzfX8e9ixFw2TrxdQVOyaMfXKtb2yNjIBf7osNVuXpjWPFh4lvkKQzCLMva2MawEvvdAmdty2aSCsKC6Kdkj5607Q9q4E6z69BZyRr06zrS678K6G86bIJmB8i3pqG9Eash8UULD8XVNfAFIiuGxtqtXM4pephgRWQfZpxD-oZQJ-gwFKlj9mL3vftkEJSD44-iL0wblPfoeA"
      },
      "expiresInSeconds": "3599",
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
