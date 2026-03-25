# Connector `datatrans` / Suite `refund_sync` / Scenario `refund_sync_with_reason`

- Service: `RefundService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"CLIENT_ERROR","message":"HTTP 405 Method Not Allowed"}}
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
date: Mon, 23 Mar 2026 18:42:00 GMT
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
  "merchant_customer_id": "mcui_e89f1f1860b549b09904dbe6",
  "customer_name": "Liam Wilson",
  "email": {
    "value": "jordan.4226@testmail.io"
  },
  "phone_number": "+919349914744",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3375 Lake Ave"
      },
      "line2": {
        "value": "6785 Oak Ave"
      },
      "line3": {
        "value": "8352 Sunset Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71084"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1653@testmail.io"
      },
      "phone_number": {
        "value": "7889998501"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2172 Main Ln"
      },
      "line2": {
        "value": "8687 Oak Dr"
      },
      "line3": {
        "value": "9034 Pine Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39077"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2994@example.com"
      },
      "phone_number": {
        "value": "1916859753"
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
date: Mon, 23 Mar 2026 18:42:00 GMT
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
  "merchant_transaction_id": "mti_f6829c75961f400687dd3a1f",
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
        "value": "Ethan Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Taylor",
    "email": {
      "value": "casey.5014@sandbox.example.com"
    },
    "id": "cust_407a2cafa32142ffb5930440",
    "phone_number": "+442042998234"
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
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3375 Lake Ave"
      },
      "line2": {
        "value": "6785 Oak Ave"
      },
      "line3": {
        "value": "8352 Sunset Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71084"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1653@testmail.io"
      },
      "phone_number": {
        "value": "7889998501"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2172 Main Ln"
      },
      "line2": {
        "value": "8687 Oak Dr"
      },
      "line3": {
        "value": "9034 Pine Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39077"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2994@example.com"
      },
      "phone_number": {
        "value": "1916859753"
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
date: Mon, 23 Mar 2026 18:42:00 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "UNAUTHORIZED",
      "message": "Invalid authentication"
    }
  },
  "statusCode": 401,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "91",
    "content-type": "application/json",
    "correlation-id": "33b5d7b9-43ea-45e4-a2b2-6130d98130b9",
    "date": "Mon, 23 Mar 2026 18:42:00 GMT",
    "p3p": "CP=\"IDC DSP COR ADM DEVi TAIi PSA PSD IVAi IVDi CONi HIS OUR IND CNT\"",
    "set-cookie": "TS0145d480=01ce483fa552e6198c49e7c30dbf685621e495951bc62d7a352be4b4bba2145fe3f4f5e6ebef3adb03e94485fc65915214e9dd4018; Path=/; Domain=.api.sandbox.datatrans.com",
    "strict-transport-security": "max-age=31536000; includeSubdomains",
    "x-xss-protection": "1; mode=block"
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

</details>
<details>
<summary>3. refund(refund_full_amount) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
```

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
  "merchant_refund_id": "mri_581eb17ba5214375b54b4fcc",
  "connector_transaction_id": "auto_generate",
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
date: Mon, 23 Mar 2026 18:42:00 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "CLIENT_ERROR",
      "message": "HTTP 404 Not Found"
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "87",
    "content-type": "application/json",
    "correlation-id": "e5da6688-3c80-498e-9a73-b3179a009264",
    "date": "Mon, 23 Mar 2026 18:42:00 GMT",
    "p3p": "CP=\"IDC DSP COR ADM DEVi TAIi PSA PSD IVAi IVDi CONi HIS OUR IND CNT\"",
    "set-cookie": "TS0145d480=01ce483fa5eab0260e5705cd29c3c0f2b81b88b4bbb7a5ca6dc47e7dce2f86d7a4f526127fe09603a6577d4362bd005499c4875136; Path=/; Domain=.api.sandbox.datatrans.com",
    "strict-transport-security": "max-age=31536000; includeSubdomains",
    "x-xss-protection": "1; mode=block"
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_sync_refund_sync_with_reason_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "refund_reason": "customer_requested"
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
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:42:01 GMT
x-request-id: refund_sync_refund_sync_with_reason_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "CLIENT_ERROR",
      "message": "HTTP 405 Method Not Allowed"
    }
  },
  "statusCode": 405,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "96",
    "content-type": "application/json",
    "correlation-id": "8aee9b30-5ca3-441b-b293-edc9be6d013e",
    "date": "Mon, 23 Mar 2026 18:42:01 GMT",
    "p3p": "CP=\"IDC DSP COR ADM DEVi TAIi PSA PSD IVAi IVDi CONi HIS OUR IND CNT\"",
    "set-cookie": "TS0145d480=01ce483fa50d3335e74db3ca64ae4c8ccecc33ed1fc57d04cd27e7184273b98304853daea492535a4eaaeba9ab16ae180bfbb2233d; Path=/; Domain=.api.sandbox.datatrans.com",
    "strict-transport-security": "max-age=31536000; includeSubdomains",
    "x-xss-protection": "1; mode=block"
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
