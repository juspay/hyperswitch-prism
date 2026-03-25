# Connector `tsys` / Suite `capture` / Scenario `capture_full_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
date: Mon, 23 Mar 2026 16:38:39 GMT
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
  "merchant_customer_id": "mcui_db4eb22333f14f2c8a07cd73e79837a2",
  "customer_name": "Noah Smith",
  "email": {
    "value": "jordan.5470@sandbox.example.com"
  },
  "phone_number": "+444225478220",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "2457 Pine St"
      },
      "line2": {
        "value": "3399 Main Ave"
      },
      "line3": {
        "value": "9015 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63622"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6199@example.com"
      },
      "phone_number": {
        "value": "3423442994"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5485 Main Ave"
      },
      "line2": {
        "value": "8596 Main Ln"
      },
      "line3": {
        "value": "1749 Market St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83484"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9550@testmail.io"
      },
      "phone_number": {
        "value": "5417228037"
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
date: Mon, 23 Mar 2026 16:38:39 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_manual_capture_credit_card) — FAIL</summary>

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
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_6fa046900a884e5486bf5f2e9402d47b",
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
        "value": "Mia Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "jordan.7085@sandbox.example.com"
    },
    "id": "cust_4aacdc3883d44b9d811668fac28bf1fb",
    "phone_number": "+911630034061"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "2457 Pine St"
      },
      "line2": {
        "value": "3399 Main Ave"
      },
      "line3": {
        "value": "9015 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63622"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6199@example.com"
      },
      "phone_number": {
        "value": "3423442994"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5485 Main Ave"
      },
      "line2": {
        "value": "8596 Main Ln"
      },
      "line3": {
        "value": "1749 Market St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83484"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9550@testmail.io"
      },
      "phone_number": {
        "value": "5417228037"
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:38:42 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "status": "AUTHORIZATION_FAILED",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "D0008",
      "message": "Possible Duplicate Request",
      "reason": "Possible Duplicate Request"
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "content-type": "application/json;charset=ISO-8859-1",
    "cross-origin-resource-policy": "cross-origin",
    "date": "Mon, 23 Mar 2026 16:38:40 GMT",
    "set-cookie": "TS01759047=01e23550880e2ef50bf0ca72cb4bfae30224ada3b9caa16c8d6677f565460f4b171913a180837d589073f5c1d21f3f56714e30d12b; Path=/; Secure; HttpOnly",
    "transfer-encoding": "chunked",
    "x-envoy-upstream-service-time": "284",
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: capture_capture_full_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_3523a280bd7a47848f0bf592a7b12d5d"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_full_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:38:42 GMT
x-request-id: capture_capture_full_amount_req

Response contents:
{
  "status": "CAPTURE_FAILED",
  "error": {
    "connectorDetails": {
      "code": "F9901",
      "message": "The value of element 'transactionID' is not valid., ",
      "reason": "The value of element 'transactionID' is not valid., "
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "content-type": "application/json;charset=ISO-8859-1",
    "cross-origin-resource-policy": "cross-origin",
    "date": "Mon, 23 Mar 2026 16:38:42 GMT",
    "set-cookie": "TS01759047=01e23550884c494cdce13d30a1a97e620dbf9638cb206574f2599ed342230564d88969340c18bd26b69d67f32e5376f25954cb4708; Path=/; Secure; HttpOnly",
    "transfer-encoding": "chunked",
    "x-envoy-upstream-service-time": "7",
    "x-frame-options": "SAMEORIGIN"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
