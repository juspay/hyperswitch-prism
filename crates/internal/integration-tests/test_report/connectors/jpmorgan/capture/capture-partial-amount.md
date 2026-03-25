# Connector `jpmorgan` / Suite `capture` / Scenario `capture_partial_amount`

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
  "merchant_transaction_id": "mti_101c8de498b94237bdd7b33b",
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
        "value": "Liam Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "jordan.4759@example.com"
    },
    "id": "cust_319690658a8d4c6aa0702bae",
    "phone_number": "+915899111962"
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
        "value": "Brown"
      },
      "line1": {
        "value": "7152 Sunset Blvd"
      },
      "line2": {
        "value": "9436 Oak Dr"
      },
      "line3": {
        "value": "4997 Main Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31136"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1924@example.com"
      },
      "phone_number": {
        "value": "3304207896"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "5712 Lake St"
      },
      "line2": {
        "value": "9583 Pine Ln"
      },
      "line3": {
        "value": "2512 Market Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12564"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1425@sandbox.example.com"
      },
      "phone_number": {
        "value": "7775627028"
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
date: Tue, 24 Mar 2026 05:47:09 GMT
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
  "merchant_capture_id": "mci_f2b4352c1a1d462b87b724b2",
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
date: Tue, 24 Mar 2026 05:47:13 GMT
x-request-id: capture_capture_partial_amount_req

Response contents:
{
  "connectorTransactionId": "auto_generate",
  "status": "CHARGED",
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 05:47:13 GMT",
    "expires": "Tue, 24 Mar 2026 05:47:13 GMT",
    "pragma": "no-cache",
    "server-timing": "ak_p; desc=\"1774331230296_398553668_1101809491_296985_6018_9_0_-\";dur=1",
    "strict-transport-security": "max-age=86400 ; includeSubDomains",
    "vary": "Accept-Encoding",
    "x-jpmc-service-type": "sandbox"
  },
  "merchantCaptureId": "mci_f2b4352c1a1d462b87b724b2",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJ0eXAiOiJKV1QiLCJraWQiOiJJR05rNSthbHVNdy9FeHQ4ejc5Wmg5ZVpZL0U9IiwiYWxnIjoiUlMyNTYifQ.eyJzdWIiOiJiOWNhMzc4NS03MzIzLTQwZTUtOTUzYS00OGM4MDc3YmFmMTciLCJjdHMiOiJPQVVUSDJfU1RBVEVMRVNTX0dSQU5UIiwiYXVkaXRUcmFja2luZ0lkIjoiZmFiNDkyOTktY2Q3YS00ZDQ3LWE4MTctZjM2Y2Y0MjFkNzU1LTQ3NzgwNjQiLCJzdWJuYW1lIjoiYjljYTM3ODUtNzMyMy00MGU1LTk1M2EtNDhjODA3N2JhZjE3IiwiaXNzIjoiaHR0cHM6Ly9pZC5wYXltZW50cy5qcG1vcmdhbi5jb206NDQzL2FtL29hdXRoMiIsInRva2VuTmFtZSI6ImFjY2Vzc190b2tlbiIsInRva2VuX3R5cGUiOiJCZWFyZXIiLCJhdXRoR3JhbnRJZCI6Ildja2dLRHlkX2pBazdRWGtlNkZOVnNPa3NCNCIsImNsaWVudF9pZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsImF1ZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsIm5iZiI6MTc3NDMzMTIzMCwiZ3JhbnRfdHlwZSI6ImNsaWVudF9jcmVkZW50aWFscyIsInNjb3BlIjpbImpwbTpwYXltZW50czpzYW5kYm94Il0sImF1dGhfdGltZSI6MTc3NDMzMTIzMCwicmVhbG0iOiIvYWxwaGEiLCJleHAiOjE3NzQzMzQ4MzAsImlhdCI6MTc3NDMzMTIzMCwiZXhwaXJlc19pbiI6MzYwMCwianRpIjoiQjlpT09mYlJaQ1psenA3ME90Q3NfR1JFU29FIn0.mtsslo3trdT8JJmbbpqPcaQulkEy7HltPKvnWSBGH96feeRBDqcQBtx4WtT7IC2mKJx8wC9C2ZRMF4Ox5kjNz9gFYaql1pFkPNzD_ieL3vXlPy8te8uNEoJWftVq-r9KzZmhOLWBNpFUy7wQxyXKxM6kDQwY5_mjXgQ7dEmdJTRYVGbLu_48O7--ekIJsnbriXPIwWzPw3cIn1dvA3hOL5itF-5mkRt7LISXBJFz4ocoya6wfGrYytl7yr6Y6tdbiJPl6P1EH9eBgUo-0uPzj7ih7ZUXJcKuymwJS5QHsUn-lgeSavF5CB4SwPlTgcISMvUTHCK184Lq1UpOXKTcxQ"
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
