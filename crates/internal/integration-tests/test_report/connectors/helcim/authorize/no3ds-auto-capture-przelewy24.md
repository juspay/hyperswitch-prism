# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_przelewy24`

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
  -H "x-request-id: authorize_no3ds_auto_capture_przelewy24_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_przelewy24_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c8ca943a60784a9fa809dd3d",
  "amount": {
    "minor_amount": 6114,
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
    "name": "Mia Taylor",
    "email": {
      "value": "riley.5392@testmail.io"
    },
    "id": "cust_62d022256cf446b99126eb8b",
    "phone_number": "+443056170645"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "2365 Pine Ln"
      },
      "line2": {
        "value": "6147 Pine Ave"
      },
      "line3": {
        "value": "2135 Sunset Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "gen_875037"
      },
      "zip_code": {
        "value": "47836"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "morgan.6194@testmail.io"
      },
      "phone_number": {
        "value": "2768261487"
      },
      "phone_country_code": "+48"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3495 Sunset Ln"
      },
      "line2": {
        "value": "1419 Sunset Blvd"
      },
      "line3": {
        "value": "4076 Sunset Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "gen_938877"
      },
      "zip_code": {
        "value": "28462"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "sam.5006@testmail.io"
      },
      "phone_number": {
        "value": "4387202040"
      },
      "phone_country_code": "+48"
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
  "description": "No3DS auto capture Przelewy24 payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_przelewy24_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_przelewy24_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:12:26 GMT
x-request-id: authorize_no3ds_auto_capture_przelewy24_req

Response contents:
{
  "merchantTransactionId": "mti_c8ca943a60784a9fa809dd3d",
  "connectorTransactionId": "46054255",
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
    "cf-ray": "9e1386268ea357ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:12:26 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "97",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=t4VXrpVN2gI63pQ6AVrtSk2i7_Hcg1uFaGviVjVgY9Q-1774332744.7237566-1.0.1.1-ec682FVyJhFpySTAlmdQVXfquPrSyhP7o92WTrZqto.fhsnLciRCxLwHApjkvMOgwqiP3R3WiGCamSWa5o7qqN2ezlY4ZlRdRj6ppkhwiHEnzJLGbxsKJ7IPPG3vJcshi60hMDVYVIIPuNZrQki6Ww; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:42:26 GMT",
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
