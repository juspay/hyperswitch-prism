# Connector `xendit` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_debit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_9b29d0b1aa5a41808c87c5b1",
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
        "value": "Mia Smith"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Wilson",
    "email": {
      "value": "riley.3266@sandbox.example.com"
    },
    "id": "cust_56c46c55b54046818b707c4a",
    "phone_number": "+11289029143"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "6345 Oak Blvd"
      },
      "line2": {
        "value": "5980 Main Ln"
      },
      "line3": {
        "value": "2135 Sunset Ln"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "16634"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "riley.3525@sandbox.example.com"
      },
      "phone_number": {
        "value": "8316119547"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6686 Sunset Ave"
      },
      "line2": {
        "value": "1947 Main Dr"
      },
      "line3": {
        "value": "8757 Main Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44686"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "jordan.5959@example.com"
      },
      "phone_number": {
        "value": "6741633404"
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
  "description": "No3DS manual capture card payment (debit)",
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:21 GMT
x-request-id: authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "26f4f6e7-e498-4cef-8f76-d922892f5f4d",
  "connectorTransactionId": "pr-9d2ffc64-6d25-42a6-92c6-aacde03d9d26",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e135149eea1054c-BOM",
    "connection": "keep-alive",
    "content-length": "1687",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:21 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "55",
    "rate-limit-reset": "46.861",
    "request-id": "69c222d3000000005475adec29869626",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=3R4Gf6o5L1jEFKNsyHdjLMNmYQ5I3MjvRkI3C4ujWdg-1774330579.512477-1.0.1.1-hTgsnczJz3Az9euRCnbQPL46cHmVz4U9q_XA72KRhrwnrJyQzbZ4o9rXG4o2WJ6oMxuwyUUx2yjUPUGB.y2WRosvtY1b2tjLdyJdpFF7gy2RNX.fT4CXYWDtjcaSvpuQ; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:21 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1598"
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
