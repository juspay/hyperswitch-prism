# Connector `rapyd` / Suite `authorize` / Scenario `no3ds_auto_capture_alipay`

- Service: `PaymentService/Authorize`
- PM / PMT: `ali_pay_redirect` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_alipay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_alipay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_0090e89cc0b04f3ba5e5165e1f2fb909",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "ali_pay_redirect": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "alex.6794@sandbox.example.com"
    },
    "id": "cust_2eff09e3db3d4846b5d23fe006445fe5",
    "phone_number": "+444017614777"
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
        "value": "Miller"
      },
      "line1": {
        "value": "8512 Pine Ave"
      },
      "line2": {
        "value": "2240 Market Rd"
      },
      "line3": {
        "value": "9896 Oak Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18679"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6920@testmail.io"
      },
      "phone_number": {
        "value": "5233044534"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "9749 Sunset St"
      },
      "line2": {
        "value": "7987 Main Ave"
      },
      "line3": {
        "value": "1657 Pine Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71703"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1165@sandbox.example.com"
      },
      "phone_number": {
        "value": "2962325243"
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
  "description": "No3DS auto capture Alipay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_alipay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_alipay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:29:19 GMT
x-request-id: authorize_no3ds_auto_capture_alipay_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "ERROR_GET_PAYMENT_METHOD_TYPE",
      "message": "ERROR",
      "reason": "The request attempted an operation that requires a payment method, but the payment method type specified is not available for this merchant or does not exist at all. Corrective action: Use a payment method type that this merchant is authorized to use."
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ed06fdf58c8e8-MRS",
    "connection": "keep-alive",
    "content-length": "440",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 16:29:19 GMT",
    "etag": "W/\"1b8-waShX6r1JjXBcFUbwC9pqLAuX28\"",
    "server": "cloudflare",
    "set-cookie": "_cfuvid=dHe.fbGVho5Xk9MT1vo1BjDnWfPe_qIFEa7V4kJJ9bU-1774283358.6948712-1.0.1.1-7rcWBjeuCMvqv73z5laUA3RCcCSrk5Dx6Hj2O.UWIPQ; HttpOnly; SameSite=None; Secure; Path=/; Domain=rapyd.net",
    "strict-transport-security": "max-age=8640000; includeSubDomains"
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
