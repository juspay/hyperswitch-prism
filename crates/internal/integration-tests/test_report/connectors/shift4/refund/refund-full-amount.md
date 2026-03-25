# Connector `shift4` / Suite `refund` / Scenario `refund_full_amount`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
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
date: Mon, 23 Mar 2026 16:31:03 GMT
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
  "merchant_customer_id": "mcui_8d7de1b15c704ed1b24e83619513569b",
  "customer_name": "Liam Miller",
  "email": {
    "value": "riley.7703@example.com"
  },
  "phone_number": "+916977116394",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6806 Sunset Ave"
      },
      "line2": {
        "value": "8960 Market St"
      },
      "line3": {
        "value": "4612 Oak Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73349"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.6321@testmail.io"
      },
      "phone_number": {
        "value": "1410196165"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4793 Market Ln"
      },
      "line2": {
        "value": "2953 Lake Ave"
      },
      "line3": {
        "value": "2926 Lake Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18098"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.9806@example.com"
      },
      "phone_number": {
        "value": "4478070067"
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
date: Mon, 23 Mar 2026 16:31:03 GMT
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
  "merchant_transaction_id": "mti_97096337afb0445ca30071ab7d65759a",
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
        "value": "Ava Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Smith",
    "email": {
      "value": "morgan.3347@example.com"
    },
    "id": "cust_2489f64fb6614a349870e5dee78686bf",
    "phone_number": "+447544990358"
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
        "value": "6806 Sunset Ave"
      },
      "line2": {
        "value": "8960 Market St"
      },
      "line3": {
        "value": "4612 Oak Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73349"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.6321@testmail.io"
      },
      "phone_number": {
        "value": "1410196165"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4793 Market Ln"
      },
      "line2": {
        "value": "2953 Lake Ave"
      },
      "line3": {
        "value": "2926 Lake Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18098"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.9806@example.com"
      },
      "phone_number": {
        "value": "4478070067"
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
date: Mon, 23 Mar 2026 16:31:03 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NO_ERROR_CODE",
      "message": "Provided API key is invalid"
    }
  },
  "statusCode": 401,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-ray": "9e0ed300099047d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:31:03 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=ls7b16fL5kEl8LizN7ZhZ4bpXMKmO5yb74K4CuL76zI-1774283463.685678-1.0.1.1-nDMAw9fHky2w3Eq32LSBkl_0sGAu2QhDZiAl_SpKI2DE0GFrPeRpusfdxvwOEFAGCrRBkcYKrDojtSy993kXuYyBsh3LjtumPO4rIP3wIc54nSENjszrRj_f6DaT6mka; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:01:03 GMT",
    "strict-transport-security": "max-age=2592000; includeSubDomains"
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
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_2632b5ef8a444583b14afbd449978609",
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
<summary>Show Response (masked)</summary>

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
date: Mon, 23 Mar 2026 16:31:03 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "NO_ERROR_CODE",
      "message": "Provided API key is invalid"
    }
  },
  "statusCode": 401,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-ray": "9e0ed301aa7b47d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:31:03 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=EyyKpKGIZr79QAfKf332gI7XSA9.XFbRCT_5g7SyAto-1774283463.9455824-1.0.1.1-DsmVUsJUubA3i2NR9ttqWK3ZQzBxobxsstJGLSueIEBgK2Rd.OuSHjti32QprH9rPmufG_KFCmGvgVMzH9lLgpnu6aY.83Zmbsp2HRk_1IvzlN0AN5uQY2dft3KKOpMx; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:01:03 GMT",
    "strict-transport-security": "max-age=2592000; includeSubDomains"
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


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
