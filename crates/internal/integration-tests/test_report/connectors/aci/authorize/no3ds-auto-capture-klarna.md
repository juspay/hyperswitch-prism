# Connector `aci` / Suite `authorize` / Scenario `no3ds_auto_capture_klarna`

- Service: `PaymentService/Authorize`
- PM / PMT: `klarna` / `-`
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
  -H "x-request-id: authorize_no3ds_auto_capture_klarna_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_klarna_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c48f33ebed4f4ceaa4ddc119",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "klarna": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "morgan.6035@example.com"
    },
    "id": "cust_538fa8f47e7f4450bc2c219e",
    "phone_number": "+444521002234"
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
        "value": "Smith"
      },
      "line1": {
        "value": "1657 Market St"
      },
      "line2": {
        "value": "5756 Main Ave"
      },
      "line3": {
        "value": "8620 Sunset Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80663"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.7792@example.com"
      },
      "phone_number": {
        "value": "2112646834"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "1494 Pine Dr"
      },
      "line2": {
        "value": "7714 Sunset Ln"
      },
      "line3": {
        "value": "9466 Market Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63912"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4228@testmail.io"
      },
      "phone_number": {
        "value": "8743434954"
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
date: Mon, 23 Mar 2026 18:24:53 GMT
x-request-id: authorize_no3ds_auto_capture_klarna_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "800.900.300",
      "message": "invalid authentication information"
    }
  },
  "statusCode": 401,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "close",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:24:53 GMT",
    "expires": "Mon, 23 Mar 2026 18:24:53 GMT",
    "pragma": "no-cache",
    "server": "ACI",
    "strict-transport-security": "max-age=63072000; includeSubdomains; preload",
    "tls-ciphers": "ECDHE-RSA-AES256-GCM-SHA384",
    "www-authenticate": "Bearer ***MASKED***, error=\"invalid_token\", error_description=\"Invalid Authorization header!\"",
    "x-application-waf-action": "allow",
    "x-content-type-options": "nosniff",
    "x-payon-ratepolicy": "auth-fail-opp",
    "x-xss-protection": "1; mode=block"
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
