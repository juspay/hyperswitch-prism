# Connector `adyen` / Suite `authorize` / Scenario `no3ds_auto_capture_bancontact`

- Service: `PaymentService/Authorize`
- PM / PMT: `bancontact_card` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"129","message":"The provided Expiry Date is not valid.: Expiry year should be a 4 digit number greater than 2000: 30","reason":"The provided Expiry Date is not valid.: Expiry year should be a 4 digit number greater than 2000: 30"}}
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_bancontact_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_864a238346a54314a28ff096",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "bancontact_card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_holder_name": {
        "value": "Ava Wilson"
      }
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "casey.7488@sandbox.example.com"
    },
    "id": "cust_7e5649a73f754b86995ce6fd",
    "phone_number": "+912288136361"
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
    "time_zone_offset_minutes": -480,
    "language": "en-US"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "9267 Sunset St"
      },
      "line2": {
        "value": "1101 Main Ave"
      },
      "line3": {
        "value": "3569 Sunset Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "63443"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "casey.9973@sandbox.example.com"
      },
      "phone_number": {
        "value": "4636977879"
      },
      "phone_country_code": "+32"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "488 Oak Ave"
      },
      "line2": {
        "value": "594 Main St"
      },
      "line3": {
        "value": "6970 Market Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "85446"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "riley.2625@testmail.io"
      },
      "phone_number": {
        "value": "6975418593"
      },
      "phone_country_code": "+32"
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
  "description": "No3DS auto capture Bancontact payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bancontact_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:26:09 GMT
x-request-id: authorize_no3ds_auto_capture_bancontact_req

Response contents:
{
  "merchantTransactionId": "W2B5X2R75X7XML65",
  "connectorTransactionId": "W2B5X2R75X7XML65",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "129",
      "message": "The provided Expiry Date is not valid.: Expiry year should be a 4 digit number greater than 2000: 30",
      "reason": "The provided Expiry Date is not valid.: Expiry year should be a 4 digit number greater than 2000: 30"
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:09 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "S93JH9PDRMN6DN65",
    "set-cookie": "JSESSIONID=830E9AE606FD6E6A19C00C799D12684B; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-61e85e500c1587cb5488a9bfd04cb33c-948b676b4e37560e-01",
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
