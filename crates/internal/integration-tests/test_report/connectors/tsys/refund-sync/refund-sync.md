# Connector `tsys` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"D7000","message":"Record(s) Not Found.","reason":"Record(s) Not Found."}}
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
date: Mon, 23 Mar 2026 16:39:08 GMT
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
  "merchant_customer_id": "mcui_219b42587c7d411d9f806d7cbdb32c42",
  "customer_name": "Ava Johnson",
  "email": {
    "value": "jordan.5766@example.com"
  },
  "phone_number": "+911391983943",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "8627 Pine Dr"
      },
      "line2": {
        "value": "9457 Main Ln"
      },
      "line3": {
        "value": "8612 Sunset Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67006"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9535@sandbox.example.com"
      },
      "phone_number": {
        "value": "3577828672"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8091 Sunset Rd"
      },
      "line2": {
        "value": "7183 Main Dr"
      },
      "line3": {
        "value": "7567 Pine St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32109"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6364@example.com"
      },
      "phone_number": {
        "value": "3949215798"
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
date: Mon, 23 Mar 2026 16:39:08 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

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
  "merchant_transaction_id": "mti_90dbaacefada4cd3a0ee49536073eaad",
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
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "morgan.9226@testmail.io"
    },
    "id": "cust_4665ba65d7f34c0dbea2ee045b84d3cf",
    "phone_number": "+11930357238"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "8627 Pine Dr"
      },
      "line2": {
        "value": "9457 Main Ln"
      },
      "line3": {
        "value": "8612 Sunset Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67006"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9535@sandbox.example.com"
      },
      "phone_number": {
        "value": "3577828672"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8091 Sunset Rd"
      },
      "line2": {
        "value": "7183 Main Dr"
      },
      "line3": {
        "value": "7567 Pine St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32109"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6364@example.com"
      },
      "phone_number": {
        "value": "3949215798"
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
date: Mon, 23 Mar 2026 16:39:09 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "76899727",
  "connectorTransactionId": "76899727",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "content-type": "application/json;charset=ISO-8859-1",
    "cross-origin-resource-policy": "cross-origin",
    "date": "Mon, 23 Mar 2026 16:39:08 GMT",
    "set-cookie": "TS01759047=01e23550883d530d580b9223bfd0a8bec57af13ffb46610aa054817c4b13da3a37a41d8f18d005c68acefc22a603b89e6020f2e9f5; Path=/; Secure; HttpOnly",
    "transfer-encoding": "chunked",
    "x-envoy-upstream-service-time": "500",
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
<summary>3. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_1fccb2ad7f434a2a856556b097112cde",
  "connector_transaction_id": "76899727",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:39:14 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "76899727",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "content-type": "application/json;charset=ISO-8859-1",
    "cross-origin-resource-policy": "cross-origin",
    "date": "Mon, 23 Mar 2026 16:39:10 GMT",
    "set-cookie": "TS01759047=01e2355088af8c0f2a0f1cc9590da649a13c4b691059b2d1a7b3b0a30a46d0c7318d9fdb31230bd8c335094fb94364594f668d3216; Path=/; Secure; HttpOnly",
    "transfer-encoding": "chunked",
    "x-envoy-upstream-service-time": "643",
    "x-frame-options": "SAMEORIGIN"
  },
  "connectorTransactionId": "76899727",
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "76899727",
  "refund_id": "76899727"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:39:15 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "D7000",
      "message": "Record(s) Not Found.",
      "reason": "Record(s) Not Found."
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "content-type": "application/json;charset=ISO-8859-1",
    "cross-origin-resource-policy": "cross-origin",
    "date": "Mon, 23 Mar 2026 16:39:15 GMT",
    "set-cookie": "TS01759047=01e2355088e2edf703d022b452ed8c073d26c6e28a5a8e23ca65aa4eb111fdd238b4a77a94f50430da0a24a692e56f6213f3971f0e; Path=/; Secure; HttpOnly",
    "transfer-encoding": "chunked",
    "x-envoy-upstream-service-time": "122",
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
