# Connector `stripe` / Suite `PaymentService/CompleteAuthorize` / Scenario `Credit Card | 3DS`

- Service: `PaymentService/Authorize`
- Scenario Key: `threeds_complete_authorize_credit_card`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:46 GMT
x-request-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Connector returned an error response with status 400
```

**Pre Requisites Executed**

<details>
<summary>1. PaymentService/Authorize(threeds_manual_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:46 GMT
x-request-id: PaymentService/Authorize_threeds_manual_capture_credit_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Connector returned an error response with status 400
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_9c79cc8eb51945a9a090419a",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000002760003184"
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
        "value": "Noah Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Miller",
    "email": {
      "value": "jordan.3802@testmail.io"
    },
    "id": "cust_9c95bed48421450fb1c14181",
    "phone_number": "+446112568284",
    "connector_customer_id": ""
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
        "value": "677 Sunset Dr"
      },
      "line2": {
        "value": "65 Lake St"
      },
      "line3": {
        "value": "5010 Oak Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "22044"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4654@example.com"
      },
      "phone_number": {
        "value": "5533731836"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1906 Pine Blvd"
      },
      "line2": {
        "value": "8360 Pine Blvd"
      },
      "line3": {
        "value": "2082 Main St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64778"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.3439@sandbox.example.com"
      },
      "phone_number": {
        "value": "9873297500"
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
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:46 GMT
x-request-id: PaymentService/Authorize_threeds_manual_capture_credit_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Connector returned an error response with status 400
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_order_id": "gen_387058",
  "merchant_transaction_id": "mti_9c79cc8eb51945a9a090419a",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000002760003184"
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
        "value": "Noah Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Miller",
    "email": {
      "value": "jordan.3802@testmail.io"
    },
    "id": "cust_9c95bed48421450fb1c14181",
    "phone_number": "+446112568284",
    "connector_customer_id": ""
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "677 Sunset Dr"
      },
      "line2": {
        "value": "65 Lake St"
      },
      "line3": {
        "value": "5010 Oak Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "22044"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4654@example.com"
      },
      "phone_number": {
        "value": "5533731836"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1906 Pine Blvd"
      },
      "line2": {
        "value": "8360 Pine Blvd"
      },
      "line3": {
        "value": "2082 Main St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64778"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.3439@sandbox.example.com"
      },
      "phone_number": {
        "value": "9873297500"
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
x-connector-request-reference-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:46 GMT
x-request-id: PaymentService/CompleteAuthorize_threeds_complete_authorize_credit_card_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Connector returned an error response with status 400
```

</details>


[Back to Connector Suite](../paymentservice-completeauthorize.md) | [Back to Overview](../../../test_overview.md)
