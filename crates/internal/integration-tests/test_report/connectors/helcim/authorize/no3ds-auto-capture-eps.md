# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_eps`

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
  -H "x-request-id: authorize_no3ds_auto_capture_eps_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_0b455114bd3c4bb4a3a8b3fa",
  "amount": {
    "minor_amount": 6109,
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
      "value": "morgan.3179@testmail.io"
    },
    "id": "cust_9e6a99d3896c404eac581dea",
    "phone_number": "+916748983016"
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
        "value": "585 Sunset Dr"
      },
      "line2": {
        "value": "7505 Market Blvd"
      },
      "line3": {
        "value": "3499 Lake Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "67614"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "morgan.6892@example.com"
      },
      "phone_number": {
        "value": "6178439748"
      },
      "phone_country_code": "+43"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2428 Market St"
      },
      "line2": {
        "value": "8238 Lake St"
      },
      "line3": {
        "value": "4214 Market St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "9"
      },
      "zip_code": {
        "value": "14262"
      },
      "country_alpha2_code": "AT",
      "email": {
        "value": "alex.5109@testmail.io"
      },
      "phone_number": {
        "value": "7251642589"
      },
      "phone_country_code": "+43"
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
  "description": "No3DS auto capture EPS payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_eps_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_eps_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:11:44 GMT
x-request-id: authorize_no3ds_auto_capture_eps_req

Response contents:
{
  "merchantTransactionId": "mti_0b455114bd3c4bb4a3a8b3fa",
  "connectorTransactionId": "46054249",
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
    "cf-ray": "9e1384c35f1157ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:30 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "92",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=.UZP1FLMyzpWi35dBEU1YOyu7iuDbRWfPDh_NNCNmPY-1774332687.8992527-1.0.1.1-xb96yr6l8VTHvLU195WC9WaxNSgP6jEjGIXm6EmCoOXwaFFgLZ2Lq9kVJA4olkCwZn_saVy9j.Sxhk3UUjScUq.pPStZ9ndtsxKOGAbDO6NMGOiIi72r65d2kABFJrdyiIVauQf.Afe8kRUT_YdI1A; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:30 GMT",
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
