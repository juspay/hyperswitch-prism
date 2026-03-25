# Connector `xendit` / Suite `get` / Scenario `sync_payment`

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
  "merchant_transaction_id": "mti_17e21194d229401d93c24f46",
  "amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "alex.2581@example.com"
    },
    "id": "cust_917a793fcb1b4550b0f01aca",
    "phone_number": "+14990054458"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "6175 Pine Ln"
      },
      "line2": {
        "value": "7572 Sunset Dr"
      },
      "line3": {
        "value": "3435 Sunset St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33106"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.6705@sandbox.example.com"
      },
      "phone_number": {
        "value": "9270885167"
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
        "value": "1439 Lake Rd"
      },
      "line2": {
        "value": "4523 Lake Ln"
      },
      "line3": {
        "value": "7737 Main Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "48479"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.9914@testmail.io"
      },
      "phone_number": {
        "value": "5708767386"
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
date: Tue, 24 Mar 2026 05:36:53 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "f6a13b61-d123-4d2c-83f9-a346a1622948",
  "connectorTransactionId": "pr-10cce253-0c99-4944-b53c-87fa86f2fa64",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1352169cb5054c-BOM",
    "connection": "keep-alive",
    "content-length": "1690",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:53 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "45",
    "rate-limit-reset": "14.122",
    "request-id": "69c222f40000000028331115fccac2ef",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=3z6V.1ot5wrwSXExeTJKzFvxo6b5Ms5hXPKsbkCctPU-1774330612.2526774-1.0.1.1-7jF1h3NFEU1dtwYjMxV6moIY52W3ohKmlDVj3ok3g9X2Ni6fYoTJY8KyUK2PWr3urUIThxJufM8PqJGvA96q5EKO0yrfTO2q.w8WgSkB5hR4Lf0rVCoTgzm82kaZNSAa; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:53 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1549"
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
  -H "x-request-id: get_sync_payment_req" \
  -H "x-connector-request-reference-id: get_sync_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "pr-10cce253-0c99-4944-b53c-87fa86f2fa64",
  "amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
date: Tue, 24 Mar 2026 05:36:54 GMT
x-request-id: get_sync_payment_req

Response contents:
{
  "status": "PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e135221eb34054c-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:54 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "59",
    "rate-limit-reset": "60",
    "request-id": "69c222f600000000464e62170cbc528d",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=2Id.sLeeSwUxSuMtK0zfpdEwB7EqNem2hCiWIVYLdGY-1774330614.0630271-1.0.1.1-OUf2LUGloDhIDRvv3JyG96fOTn0p.A7qA4IAaUfFwHiI47ULWoxm5nAQ1pajUGqz7Q0oy2rdPoO8kQNbTLR4nDU_idmXFWvWkxCp9VRkr6XPhumTmkPzAdkecbyuxgrU; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:54 GMT",
    "transfer-encoding": "chunked",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "85"
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


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
