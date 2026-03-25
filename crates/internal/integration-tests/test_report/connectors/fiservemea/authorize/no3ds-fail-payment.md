# Connector `fiservemea` / Suite `authorize` / Scenario `no3ds_fail_payment`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to exist
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_fail_payment_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_fail_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_2e2e4eac77cf4961aca943cd",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000000000000002"
      },
      "card_exp_month": {
        "value": "01"
      },
      "card_exp_year": {
        "value": "35"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "morgan.5912@sandbox.example.com"
    },
    "id": "cust_dd7cdeb5b8b04d3a8026ff73",
    "phone_number": "+919736429788"
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
        "value": "1031 Main St"
      },
      "line2": {
        "value": "5141 Market Ln"
      },
      "line3": {
        "value": "2104 Lake Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "84188"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.9659@testmail.io"
      },
      "phone_number": {
        "value": "3853149792"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8559 Market St"
      },
      "line2": {
        "value": "776 Market Rd"
      },
      "line3": {
        "value": "4127 Lake Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "78265"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9188@sandbox.example.com"
      },
      "phone_number": {
        "value": "8860103384"
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
  "description": "No3DS fail payment flow",
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
x-connector-request-reference-id: authorize_no3ds_fail_payment_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_fail_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:00:34 GMT
x-request-id: authorize_no3ds_fail_payment_req

Response contents:
{
  "merchantTransactionId": "4fda1972-6ad4-4dba-92a3-e26bc422dcda",
  "connectorTransactionId": "84649326156",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "4fda1972-6ad4-4dba-92a3-e26bc422dcda",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:00:34 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335634; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:00:34 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "273",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "4276d8bd-2be2-4a1a-a300-b0f7c692b937",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "acI2khdoNHd8qrsBCPFlWwAAA40",
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
