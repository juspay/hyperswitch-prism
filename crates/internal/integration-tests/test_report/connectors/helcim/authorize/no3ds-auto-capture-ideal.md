# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_ideal`

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
  -H "x-request-id: authorize_no3ds_auto_capture_ideal_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_4a3e5edb2b704c42829e659d",
  "amount": {
    "minor_amount": 6112,
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
    "name": "Liam Smith",
    "email": {
      "value": "casey.4763@sandbox.example.com"
    },
    "id": "cust_ac9d562e5ebc46f9b3bc8fba",
    "phone_number": "+915833253631"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "3029 Lake Dr"
      },
      "line2": {
        "value": "5702 Sunset Blvd"
      },
      "line3": {
        "value": "4512 Lake Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "94872"
      },
      "country_alpha2_code": "NL",
      "email": {
        "value": "morgan.4372@example.com"
      },
      "phone_number": {
        "value": "7414983044"
      },
      "phone_country_code": "+31"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2545 Pine Blvd"
      },
      "line2": {
        "value": "8079 Pine St"
      },
      "line3": {
        "value": "5351 Market Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "57195"
      },
      "country_alpha2_code": "NL",
      "email": {
        "value": "jordan.1738@testmail.io"
      },
      "phone_number": {
        "value": "8183913001"
      },
      "phone_country_code": "+31"
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
  "description": "No3DS auto capture iDEAL payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_ideal_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:12:22 GMT
x-request-id: authorize_no3ds_auto_capture_ideal_req

Response contents:
{
  "merchantTransactionId": "mti_4a3e5edb2b704c42829e659d",
  "connectorTransactionId": "46054253",
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
    "cf-ray": "9e1385f80b2257ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:12:19 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "99",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=B91MWdN_Ji5bbIPW5uzjPptmuHptGK3NRUzt1rkgRqI-1774332737.2820826-1.0.1.1-XEWkqhfdz47JSHBGLOkPHM7M5jyF_lCIGTzX5EKcVV9Siv_Uis_a_Qrd2pPw9pWK.NsqsJGQQCq1vIo26Gz8J1fcr0LNK56yELS9wBSfEEsJhqPplXzaQVs8Pwp91el4S1OW6lQ2W.xHCBS_pu.HFA; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:42:19 GMT",
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
