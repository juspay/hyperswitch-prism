# Connector `checkout` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"no_processor_configured_for_card_scheme","message":"no_processor_configured_for_card_scheme","reason":"no_processor_configured_for_card_scheme"}}
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_d258c44302e645718110828c",
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
        "value": "Emma Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Taylor",
    "email": {
      "value": "riley.9140@example.com"
    },
    "id": "cust_54729ca2e04041afbe70516a",
    "phone_number": "+918448948639"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3674 Lake Dr"
      },
      "line2": {
        "value": "306 Pine St"
      },
      "line3": {
        "value": "6361 Pine Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "68743"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8016@sandbox.example.com"
      },
      "phone_number": {
        "value": "2502966558"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "2624 Oak St"
      },
      "line2": {
        "value": "112 Pine Rd"
      },
      "line3": {
        "value": "8853 Sunset St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "90727"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3602@sandbox.example.com"
      },
      "phone_number": {
        "value": "1423673502"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
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
x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:39:10 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "274de21b-bc98-405c-a230-801233873085",
  "connectorTransactionId": "274de21b-bc98-405c-a230-801233873085",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "no_processor_configured_for_card_scheme",
      "message": "no_processor_configured_for_card_scheme",
      "reason": "no_processor_configured_for_card_scheme"
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "cko-request-id": "274de21b-bc98-405c-a230-801233873085",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "143",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:10 GMT",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
