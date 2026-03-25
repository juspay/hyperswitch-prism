# Connector `hipay` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
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
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_00f5fd8ae1cf459f9693f7ee",
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
        "value": "Mia Wilson"
      },
      "card_type": "credit",
      "card_network": "VISA"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Smith",
    "email": {
      "value": "alex.9258@example.com"
    },
    "id": "cust_6bab780c0c3b4e4499e5532a",
    "phone_number": "+443672501481"
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
        "value": "Brown"
      },
      "line1": {
        "value": "8859 Pine Blvd"
      },
      "line2": {
        "value": "9900 Pine Ln"
      },
      "line3": {
        "value": "8851 Sunset St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99124"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5007@sandbox.example.com"
      },
      "phone_number": {
        "value": "8942107833"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4429 Lake Dr"
      },
      "line2": {
        "value": "6606 Main Ln"
      },
      "line3": {
        "value": "2448 Main Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55955"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1070@testmail.io"
      },
      "phone_number": {
        "value": "5540023050"
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
date: Tue, 24 Mar 2026 05:46:03 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "0",
      "message": "CardToken is required",
      "reason": "CardToken is required"
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "",
    "access-control-allow-origin": "",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "close",
    "content-length": "63",
    "content-type": "application/json; encoding=UTF-8",
    "date": "Tue, 24 Mar 2026 05:46:03 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "pragma": "no-cache"
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
