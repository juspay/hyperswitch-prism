# Connector `nuvei` / Suite `PaymentService/Authorize` / Scenario `Przelewy24 | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_przelewy24`
- PM / PMT: `przelewy24` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Fri, 10 Apr 2026 21:20:53 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
Sent 1 request and received 0 responses

ERROR:
  Code: FailedPrecondition
  Message: Payment method not supported is not supported by nuvei
```

**Pre Requisites Executed**

<details>
<summary>1. MerchantAuthenticationService/CreateServerSessionAuthenticationToken(create_session_basic) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_basic_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_basic_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken <<'JSON'
{
  "test_mode": true,
  "merchant_server_session_id": "gen_166470",
  "payment": {
    "amount": {
      "minor_amount": 10000,
      "currency": "USD"
    }
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Create a server-side session with the connector. Establishes session state
// for multi-step operations like 3DS verification or wallet authorization.
rpc CreateServerSessionAuthenticationToken ( .types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_basic_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_basic_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 10 Apr 2026 21:20:53 GMT
x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_basic_req

Response contents:
{
  "statusCode": 200,
  "sessionToken": ***MASKED***"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_76c98c7c4d0447d3a1525ed8",
  "amount": {
    "minor_amount": 10000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "przelewy24": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "riley.9729@example.com"
    },
    "id": "cust_cae32ab9c09b4849b689b19d",
    "phone_number": "+441911706828"
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "2695 Main Blvd"
      },
      "line2": {
        "value": "4042 Market Ln"
      },
      "line3": {
        "value": "3978 Main St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "gen_242465"
      },
      "zip_code": {
        "value": "28916"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "riley.3411@sandbox.example.com"
      },
      "phone_number": {
        "value": "2070765809"
      },
      "phone_country_code": "+48"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1163 Oak Ave"
      },
      "line2": {
        "value": "8495 Lake Rd"
      },
      "line3": {
        "value": "3091 Sunset Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "gen_512340"
      },
      "zip_code": {
        "value": "86329"
      },
      "country_alpha2_code": "PL",
      "email": {
        "value": "riley.6729@sandbox.example.com"
      },
      "phone_number": {
        "value": "5601165708"
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
  "locale": "en-US",
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
  "order_details": []
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Fri, 10 Apr 2026 21:20:53 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_przelewy24_req
Sent 1 request and received 0 responses

ERROR:
  Code: FailedPrecondition
  Message: Payment method not supported is not supported by nuvei
```

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
