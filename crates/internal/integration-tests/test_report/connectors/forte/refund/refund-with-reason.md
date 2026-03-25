# Connector `forte` / Suite `refund` / Scenario `refund_with_reason`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_with_reason_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:49:27 GMT
x-request-id: refund_refund_with_reason_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector meta data not found
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — FAIL</summary>

**Dependency Error**

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:49:27 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_67409a85b4b94efd8452d13b",
  "customer_name": "Noah Wilson",
  "email": {
    "value": "jordan.5390@testmail.io"
  },
  "phone_number": "+16658928293",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9505 Oak Ln"
      },
      "line2": {
        "value": "3294 Pine St"
      },
      "line3": {
        "value": "3192 Market Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "11228"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3344@example.com"
      },
      "phone_number": {
        "value": "2547230485"
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
        "value": "1450 Lake Blvd"
      },
      "line2": {
        "value": "4552 Pine Ln"
      },
      "line3": {
        "value": "3021 Main Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "46533"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8341@sandbox.example.com"
      },
      "phone_number": {
        "value": "5975144126"
      },
      "phone_country_code": "+91"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:49:27 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_a3b416a08b374dcd9111ce32",
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
        "value": "Liam Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Taylor",
    "email": {
      "value": "casey.8284@example.com"
    },
    "id": "cust_774ea1f8f7ac4767a8c6d0be",
    "phone_number": "+15559937763"
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
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9505 Oak Ln"
      },
      "line2": {
        "value": "3294 Pine St"
      },
      "line3": {
        "value": "3192 Market Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "11228"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3344@example.com"
      },
      "phone_number": {
        "value": "2547230485"
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
        "value": "1450 Lake Blvd"
      },
      "line2": {
        "value": "4552 Pine Ln"
      },
      "line3": {
        "value": "3021 Main Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "46533"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8341@sandbox.example.com"
      },
      "phone_number": {
        "value": "5975144126"
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
  "description": "No3DS auto capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:49:27 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "No error code",
      "message": "Error[1]: The content in the request produced errors while parsing. Check that the content is correctly formatted for the Content-Type provided. Error[2]: Could not find member 'card_type' on object of type 'transaction'.",
      "reason": "Error[1]: The content in the request produced errors while parsing. Check that the content is correctly formatted for the Content-Type provided. Error[2]: Could not find member 'card_type' on object of type 'transaction'."
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "278",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:49:27 GMT",
    "server": "Kestrel",
    "www-authenticate": "Basic realm=\"https://sandbox.forte.net/api/v3/organizations/org_438530/locations/loc_316671/transactions\"",
    "x-api-mode": "modern"
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_with_reason_req" \
  -H "x-connector-request-reference-id: refund_refund_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_197513140eed45e792b2104d",
  "connector_transaction_id": "auto_generate",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "reason": "customer_requested"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_with_reason_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:49:27 GMT
x-request-id: refund_refund_with_reason_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector meta data not found
```

</details>


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
