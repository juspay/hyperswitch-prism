# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_google_pay_encrypted`

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
  -H "x-request-id: authorize_no3ds_auto_capture_google_pay_encrypted_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_google_pay_encrypted_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_aa3f2cd991864e94a6d0dcbc",
  "amount": {
    "minor_amount": 6111,
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
    "name": "Emma Miller",
    "email": {
      "value": "jordan.9126@sandbox.example.com"
    },
    "id": "cust_851a8f55e13944ad97bec99f",
    "phone_number": "+19060262421"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "211 Oak St"
      },
      "line2": {
        "value": "1837 Main Rd"
      },
      "line3": {
        "value": "9581 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "69793"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7168@testmail.io"
      },
      "phone_number": {
        "value": "7947029691"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8164 Oak Ln"
      },
      "line2": {
        "value": "5204 Market Ave"
      },
      "line3": {
        "value": "3965 Oak Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91199"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9515@sandbox.example.com"
      },
      "phone_number": {
        "value": "8023225546"
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
  "description": "No3DS auto capture Google Pay (encrypted token)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_google_pay_encrypted_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_google_pay_encrypted_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:12:17 GMT
x-request-id: authorize_no3ds_auto_capture_google_pay_encrypted_req

Response contents:
{
  "merchantTransactionId": "mti_aa3f2cd991864e94a6d0dcbc",
  "connectorTransactionId": "46054252",
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
    "cf-ray": "9e1385eaf84857ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:12:17 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "100",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=tuoVQyFO3vqSEzazaPidr5b60e.XP2dbMRLPF_pHMu4-1774332735.1993618-1.0.1.1-S5zpJnALkkOJtPc5aGdurp7zPnd49wRkzB0BoqI6BsnbIGXULmB9vqjsxxRPyu_oHshC7mWaAJU4ag4hpWX5_1jZzjfrPzt778KEkQ01T39swt.hJhlLgAM3lv.kenJgx2PBcPwe29YsNghvw4fKVQ; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:42:17 GMT",
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
