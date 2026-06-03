# Connector `nuvei` / Suite `PaymentService/Authorize` / Scenario `Credit Card | No 3DS | Manual Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_manual_capture_credit_card`
- PM / PMT: `card` / `credit`
- Result: `PASS`

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
  "merchant_server_session_id": "gen_553014",
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
date: Fri, 10 Apr 2026 21:20:56 GMT
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
  -H "x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_3a0befbb1f0140e1afc1244a",
  "amount": {
    "minor_amount": 10000,
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
      "card_holder_name": {
        "value": "Noah Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Smith",
    "email": {
      "value": "riley.9594@testmail.io"
    },
    "id": "cust_494c9886d11c4f8088bedd54",
    "phone_number": "+14046980254"
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "6830 Oak Blvd"
      },
      "line2": {
        "value": "9644 Lake Rd"
      },
      "line3": {
        "value": "1364 Pine Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71967"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9297@example.com"
      },
      "phone_number": {
        "value": "5042834507"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7225 Market Rd"
      },
      "line2": {
        "value": "4630 Main St"
      },
      "line3": {
        "value": "6275 Main Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18484"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5203@example.com"
      },
      "phone_number": {
        "value": "7006894519"
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
  "description": "No3DS manual capture card payment (credit)",
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 10 Apr 2026 21:20:57 GMT
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_3a0befbb1f0140e1afc1244a",
  "connectorTransactionId": "8110000000027212066",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-methods": "GET, POST",
    "access-control-allow-origin": "*",
    "connection": "keep-alive",
    "content-length": "1024",
    "content-type": "application/json;charset=UTF-8",
    "date": "Fri, 10 Apr 2026 21:20:57 GMT",
    "p3p": "CP=\"ALL ADM DEV PSAi COM NAV OUR OTR STP IND DEM\"",
    "server": "nginx",
    "set-cookie": ***MASKED***"
  },
  "rawConnectorResponse": "***MASKED***",
  "rawConnectorRequest": "***MASKED***"


Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
