# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_giropay`

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
  -H "x-request-id: authorize_no3ds_auto_capture_giropay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_giropay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_d38197cd9ec64339afbf6a1b",
  "amount": {
    "minor_amount": 6110,
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
    "name": "Mia Smith",
    "email": {
      "value": "riley.1361@sandbox.example.com"
    },
    "id": "cust_d47296dd1b8c4652971f2478",
    "phone_number": "+12025483894"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "8844 Main Rd"
      },
      "line2": {
        "value": "6977 Pine Rd"
      },
      "line3": {
        "value": "3147 Pine Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "49463"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "morgan.3458@example.com"
      },
      "phone_number": {
        "value": "1077490578"
      },
      "phone_country_code": "+49"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "1000 Lake St"
      },
      "line2": {
        "value": "8328 Oak Rd"
      },
      "line3": {
        "value": "3162 Main Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "BY"
      },
      "zip_code": {
        "value": "95301"
      },
      "country_alpha2_code": "DE",
      "email": {
        "value": "alex.4839@testmail.io"
      },
      "phone_number": {
        "value": "2373389848"
      },
      "phone_country_code": "+49"
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
  "description": "No3DS auto capture Giropay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_giropay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_giropay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:12:15 GMT
x-request-id: authorize_no3ds_auto_capture_giropay_req

Response contents:
{
  "merchantTransactionId": "mti_d38197cd9ec64339afbf6a1b",
  "connectorTransactionId": "46054250",
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
    "cf-ray": "9e13852b990857ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:46 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "91",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=slluhavIjQPjdPcM4imEjgvg5lyLGL3TuBfnYk2OmU0-1774332704.5803802-1.0.1.1-JM4_qlyHWOJbWy0_kCtUhfBqej2OYDOszLuhW3WYW2ST0gk4UdsKuUFxNgyVJbXuT6kq8rfufet1Oe6Qtee.mON.7TnD.Og3cdBLNCRaEnbVvXN2ojJ4l2smbxkixakOv4Q7KVK25sTLQIsHfBEolw; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:46 GMT",
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
