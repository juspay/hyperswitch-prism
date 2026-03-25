# Connector `powertranz` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

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
  "merchant_transaction_id": "mti_9f8902d5696949e0bcc48eac",
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
        "value": "Emma Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Taylor",
    "email": {
      "value": "sam.7946@sandbox.example.com"
    },
    "id": "cust_2ca9d4785b954bdb8e8c33d2",
    "phone_number": "+914483867103"
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
        "value": "210 Pine Ln"
      },
      "line2": {
        "value": "5013 Pine Ave"
      },
      "line3": {
        "value": "1006 Oak Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35462"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2325@testmail.io"
      },
      "phone_number": {
        "value": "3125339616"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9498 Market Blvd"
      },
      "line2": {
        "value": "3556 Pine Rd"
      },
      "line3": {
        "value": "854 Lake Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96528"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7926@testmail.io"
      },
      "phone_number": {
        "value": "7564014376"
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
date: Tue, 24 Mar 2026 07:06:58 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "cd505857-f6bb-4061-8808-ae6245b71faf",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d60c9d0947e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:58 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=6Cs4kHMXDhn96DfWtaQCcUmn3XVOYaH5FEsmFjz7nf8-1774336017.3763523-1.0.1.1-wtjZpRZEBwSDROXLQJ6GPqh3uXSPHY8BLClstt0zh0VS.9_DFchC9uanLSXjDTgJSe2aUfZYN8iO.ILI_rkuNKeMcdTntIM_cFsy.Wq6yFe2F3bpw1oSeD0iLUI20TZB; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:58 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "cd505857-f6bb-4061-8808-ae6245b71faf",
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
date: Tue, 24 Mar 2026 07:06:59 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "connectorTransactionId": "cd505857-f6bb-4061-8808-ae6245b71faf",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d6159b8247e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:59 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=0UQ9irMG7EcWUX3sRZ2KbJNiJvptsWzNNhXIGGGi01U-1774336018.8176937-1.0.1.1-ZcWVAI0JgHZN8rH8M.oX6kZ.6wU_sKGdniCrzXLmjal4XaO.Mm18aSFL2EX9lPQUfIRAflkd7BkFsnGnvFw75_pOALAkdXaXY8skkQZVsKonqZF2KT9tMEuAYtdU5MXT; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:59 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
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
