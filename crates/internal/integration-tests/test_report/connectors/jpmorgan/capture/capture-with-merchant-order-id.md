# Connector `jpmorgan` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_b0f2c8c366764abbbd479308",
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
        "value": "Ethan Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Miller",
    "email": {
      "value": "alex.2472@sandbox.example.com"
    },
    "id": "cust_14537de7cba6434fbe6c3167",
    "phone_number": "+442977335714"
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
        "value": "Smith"
      },
      "line1": {
        "value": "3951 Oak St"
      },
      "line2": {
        "value": "5683 Lake Ln"
      },
      "line3": {
        "value": "4937 Oak Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80384"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9037@sandbox.example.com"
      },
      "phone_number": {
        "value": "1749803855"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "4032 Main Dr"
      },
      "line2": {
        "value": "6455 Pine Ln"
      },
      "line3": {
        "value": "4124 Sunset Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56150"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6543@sandbox.example.com"
      },
      "phone_number": {
        "value": "1695316985"
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
date: Tue, 24 Mar 2026 05:47:13 GMT
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_8f7439a5e3e1463b818fd38d",
  "merchant_order_id": "gen_569412",
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
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:47:16 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "auto_generate",
  "status": "CHARGED",
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 05:47:16 GMT",
    "expires": "Tue, 24 Mar 2026 05:47:16 GMT",
    "pragma": "no-cache",
    "server-timing": "ak_p; desc=\"1774331233903_398553668_1101848013_272298_7661_8_0_-\";dur=1",
    "strict-transport-security": "max-age=86400 ; includeSubDomains",
    "vary": "Accept-Encoding",
    "x-jpmc-service-type": "sandbox"
  },
  "merchantCaptureId": "mci_8f7439a5e3e1463b818fd38d",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "eyJ0eXAiOiJKV1QiLCJraWQiOiJJR05rNSthbHVNdy9FeHQ4ejc5Wmg5ZVpZL0U9IiwiYWxnIjoiUlMyNTYifQ.eyJzdWIiOiJiOWNhMzc4NS03MzIzLTQwZTUtOTUzYS00OGM4MDc3YmFmMTciLCJjdHMiOiJPQVVUSDJfU1RBVEVMRVNTX0dSQU5UIiwiYXVkaXRUcmFja2luZ0lkIjoiZTAzMmRkMTAtZTY4Yi00OTk2LWE4MzktMmRkYWRjNjQ3YmU3LTQ3OTAwOTEiLCJzdWJuYW1lIjoiYjljYTM3ODUtNzMyMy00MGU1LTk1M2EtNDhjODA3N2JhZjE3IiwiaXNzIjoiaHR0cHM6Ly9pZC5wYXltZW50cy5qcG1vcmdhbi5jb206NDQzL2FtL29hdXRoMiIsInRva2VuTmFtZSI6ImFjY2Vzc190b2tlbiIsInRva2VuX3R5cGUiOiJCZWFyZXIiLCJhdXRoR3JhbnRJZCI6InZRYVdKOEk3bXN2eVlLY2U1d3lLazV3RHZYZyIsImNsaWVudF9pZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsImF1ZCI6ImI5Y2EzNzg1LTczMjMtNDBlNS05NTNhLTQ4YzgwNzdiYWYxNyIsIm5iZiI6MTc3NDMzMTIzMywiZ3JhbnRfdHlwZSI6ImNsaWVudF9jcmVkZW50aWFscyIsInNjb3BlIjpbImpwbTpwYXltZW50czpzYW5kYm94Il0sImF1dGhfdGltZSI6MTc3NDMzMTIzMywicmVhbG0iOiIvYWxwaGEiLCJleHAiOjE3NzQzMzQ4MzMsImlhdCI6MTc3NDMzMTIzMywiZXhwaXJlc19pbiI6MzYwMCwianRpIjoiYXJzZ0VhVnlPSmFzZlZvX01oSVpvUzhlRVhzIn0.EUT95362vItvJPd6YfEEVwB62HawWqBSXdTLbOcV0bhg6UYm14Zoc0-qPxhXaMqqNDneJXOH4Kqq6DVa-fkeW1zUIvx2BQyPzvaWXUXgdkAYxfRKQY6lCAjtOcMt4aDl4gbB1NOCrzkYRhdbor22AxYoeYHl3uFjQpgCYMmLIx2ys2y60nvzGuISQsgw3smUofn0b802-zXbG5NGv3p3x04KWOUtN7ooIJgf2P_MCKUVCKZuI55dny9ZcNRa869azlqHvUpxJl7SYWz1aa4gNTv_gsWUba0t7gq9Yfx3ypZfWb8wP5itF2OFyxy8snefllm1SPpPbaqeYJduzicV3g"
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
