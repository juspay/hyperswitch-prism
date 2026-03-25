# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_klarna`

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
  -H "x-request-id: authorize_no3ds_auto_capture_klarna_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_d0be92f2236641acbb1819a9",
  "amount": {
    "minor_amount": 6113,
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
    "name": "Ava Smith",
    "email": {
      "value": "jordan.7145@sandbox.example.com"
    },
    "id": "cust_37fc72cb3c104309a1eff575",
    "phone_number": "+919456007605"
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
        "value": "Brown"
      },
      "line1": {
        "value": "7540 Oak St"
      },
      "line2": {
        "value": "2951 Pine St"
      },
      "line3": {
        "value": "5043 Main Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73146"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.4567@example.com"
      },
      "phone_number": {
        "value": "4494241896"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9728 Main Blvd"
      },
      "line2": {
        "value": "7046 Pine Ln"
      },
      "line3": {
        "value": "8051 Pine Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21971"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1043@testmail.io"
      },
      "phone_number": {
        "value": "3328898061"
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
  "description": "No3DS auto capture Klarna payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_klarna_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:12:24 GMT
x-request-id: authorize_no3ds_auto_capture_klarna_req

Response contents:
{
  "merchantTransactionId": "mti_d0be92f2236641acbb1819a9",
  "connectorTransactionId": "46054254",
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
    "cf-ray": "9e138619ba7257ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:12:24 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "98",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=cgO4RovT57WF4Ty7paJJyOwezT.eRRvUWFQ31KIO5l0-1774332742.6814213-1.0.1.1-d9mRz22lEcaRmpLEl4wEQZ9VpQCkpQh0LFBd7z7YxVJqQqmyr7dEERKHKX3Uhq_waHm9RSQTiERD1fAmnt8u2ZCfKH1cDQ4af85IsTsU4tuBNntXrWQn6DQdz0hw0BK8r_0lX5n9cqbpms.wTC_Dgg; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:42:24 GMT",
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
