# Connector `adyen` / Suite `authorize` / Scenario `no3ds_auto_capture_affirm`

- Service: `PaymentService/Authorize`
- PM / PMT: `affirm` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"No error code","message":"No error message"}}
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_affirm_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_affirm_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_cf528ae6585a47a484c2e541",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "affirm": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Smith",
    "email": {
      "value": "alex.8213@example.com"
    },
    "id": "cust_0102ae5e73c445ddaa4e8ef5",
    "phone_number": "+441611949469"
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
        "value": "Brown"
      },
      "line1": {
        "value": "8252 Pine Ave"
      },
      "line2": {
        "value": "6419 Main Ave"
      },
      "line3": {
        "value": "8864 Main Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66880"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7683@example.com"
      },
      "phone_number": {
        "value": "2249611945"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "7861 Main Ln"
      },
      "line2": {
        "value": "5388 Market Blvd"
      },
      "line3": {
        "value": "5792 Market Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "23003"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1550@testmail.io"
      },
      "phone_number": {
        "value": "3384815787"
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
  "description": "No3DS auto capture Affirm payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_affirm_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_affirm_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:26:07 GMT
x-request-id: authorize_no3ds_auto_capture_affirm_req

Response contents:
{
  "merchantTransactionId": "J9X99LJCJ3Z6BQ75",
  "connectorTransactionId": "J9X99LJCJ3Z6BQ75",
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "No error code",
      "message": "No error message"
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:07 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "FZHXWBW67SP75X65",
    "set-cookie": "JSESSIONID=686BAC8A99D405E6B9BBD12BEA750B3B; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-7dcac8c0f58c95376fec79dcce69f015-7b94c63e01fcc735-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
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
