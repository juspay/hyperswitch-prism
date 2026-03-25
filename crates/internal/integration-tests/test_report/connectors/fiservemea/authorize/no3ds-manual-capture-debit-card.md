# Connector `fiservemea` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

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
  "merchant_transaction_id": "mti_3ec3360b1fb448038c5b4fdd",
  "amount": {
    "minor_amount": 6000,
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
        "value": "Ethan Wilson"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Johnson",
    "email": {
      "value": "sam.9202@example.com"
    },
    "id": "cust_ac29c5ec1a8843449d78ed70",
    "phone_number": "+917888912374"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "581 Main Ave"
      },
      "line2": {
        "value": "9761 Main St"
      },
      "line3": {
        "value": "400 Lake Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "68317"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5357@testmail.io"
      },
      "phone_number": {
        "value": "6357877075"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3812 Main Ave"
      },
      "line2": {
        "value": "7197 Lake Blvd"
      },
      "line3": {
        "value": "324 Lake St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "20455"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3671@example.com"
      },
      "phone_number": {
        "value": "4416292179"
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
date: Tue, 24 Mar 2026 07:00:39 GMT
x-request-id: authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "f507d98f-92b8-4396-84de-da8dd2328649",
  "connectorTransactionId": "84649326158",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "f507d98f-92b8-4396-84de-da8dd2328649",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:00:36 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335636; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:00:36 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "286",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "64a6f82a-c6a0-4598-9051-cb39457cd624",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "acI2lBdoNHd8qrsBCPFlXQAAA4U",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorFeatureData": {
    "value": "{\"token_reusable\":\"true\"}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
