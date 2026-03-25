# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_affirm`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_affirm_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_affirm_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_5d07776cc163408eaf1e9659",
  "amount": {
    "minor_amount": 6102,
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
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Wilson",
    "email": {
      "value": "riley.4488@example.com"
    },
    "id": "cust_329f620f6b7945ed8bb857a5",
    "phone_number": "+443263611160"
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
        "value": "381 Sunset Blvd"
      },
      "line2": {
        "value": "6585 Pine Ln"
      },
      "line3": {
        "value": "3141 Oak Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59615"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4164@testmail.io"
      },
      "phone_number": {
        "value": "4116279291"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5558 Lake St"
      },
      "line2": {
        "value": "8116 Sunset Ln"
      },
      "line3": {
        "value": "4864 Main Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "69574"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3422@testmail.io"
      },
      "phone_number": {
        "value": "9624433163"
      },
      "phone_country_code": "+1"
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
  "description": "No3DS auto capture Affirm payment",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_auto_capture_affirm_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_affirm_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:11:13 GMT
x-request-id: authorize_no3ds_auto_capture_affirm_req

Response contents:
{
  "merchantTransactionId": "mti_5d07776cc163408eaf1e9659",
  "connectorTransactionId": "46054242",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Origin, Content-Type, X-Auth-Token, js-token, user-token, business-id",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Origin, Content-Type, X-Auth-Token, js-token",
    "access-control-max-age": "600",
    "alt-svc": "h3=\":443\"; ma=86400",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13845e3c7157ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:13 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "99",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=Cdh4bYDaHHMOenhxZibmtc6e714exDt4_VjfClYUmBE-1774332671.715979-1.0.1.1-gjYXs6YXmXx_YLerrODQLMBu7_MNSiLOcnNPh6kuhg.ItovHBr0RClfKOVjL4nnE5kHBAk6zjDJ_GEUXnGfDZ2wKzxlLgne6IHR7Fxo36S.lCCo0MlFAIIW76gtLqyMEtNeOLuydauRlzyg68rdE5A; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:13 GMT",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked"
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


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
