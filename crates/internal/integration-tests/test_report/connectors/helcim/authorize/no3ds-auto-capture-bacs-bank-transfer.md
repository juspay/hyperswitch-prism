# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_bacs_bank_transfer`

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
  -H "x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bacs_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_e6f0d82d6ce34812a87e71fe",
  "amount": {
    "minor_amount": 6105,
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
    "name": "Emma Johnson",
    "email": {
      "value": "alex.3816@example.com"
    },
    "id": "cust_186e726294914d31813e27ee",
    "phone_number": "+446901649043"
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
        "value": "Smith"
      },
      "line1": {
        "value": "8463 Pine Ave"
      },
      "line2": {
        "value": "5389 Sunset Ln"
      },
      "line3": {
        "value": "5574 Oak St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "29637"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "casey.2328@testmail.io"
      },
      "phone_number": {
        "value": "8465202327"
      },
      "phone_country_code": "+44"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2915 Main Ln"
      },
      "line2": {
        "value": "7143 Lake Dr"
      },
      "line3": {
        "value": "7542 Lake St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "94351"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "riley.7652@sandbox.example.com"
      },
      "phone_number": {
        "value": "7879009721"
      },
      "phone_country_code": "+44"
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
  "description": "No3DS BACS bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bacs_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:11:21 GMT
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req

Response contents:
{
  "merchantTransactionId": "mti_e6f0d82d6ce34812a87e71fe",
  "connectorTransactionId": "46054245",
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
    "cf-ray": "9e138484ba9857ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:21 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "96",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=QBlR6V6uhu4KnvheKRfi5I98SIWlyFAc_e_BlQWJBHU-1774332677.8790414-1.0.1.1-6FDdcFVkqDMx_XQTgPOzLZsE9iFR8ljj3jpD.hdayfnMq3k86z6Z1RkTqTtgUIb.rrJdqqqL6_0QAbBZ0Juso1kCeGxoWHM2.nX5Vv4zweDE3ORqBdQvXSHSL6UoDFTZDoE5KCLjMogwzmOJ66OXeQ; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:21 GMT",
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
