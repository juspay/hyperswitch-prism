# Connector `shift4` / Suite `capture` / Scenario `capture_full_amount`

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
date: Mon, 23 Mar 2026 16:30:59 GMT
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
  "merchant_customer_id": "mcui_4394e7d4c2cf4c1a9edc54995972990b",
  "customer_name": "Ethan Taylor",
  "email": {
    "value": "jordan.6020@testmail.io"
  },
  "phone_number": "+912952220453",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3099 Main Ave"
      },
      "line2": {
        "value": "1496 Pine Blvd"
      },
      "line3": {
        "value": "4010 Market Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83154"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5228@example.com"
      },
      "phone_number": {
        "value": "1676089766"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3878 Main Rd"
      },
      "line2": {
        "value": "4250 Market Ln"
      },
      "line3": {
        "value": "2740 Sunset Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55849"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8542@testmail.io"
      },
      "phone_number": {
        "value": "2488934791"
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
date: Mon, 23 Mar 2026 16:30:59 GMT
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
  "merchant_transaction_id": "mti_1b65c586a8e84e05b38de2e0ae02bd98",
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
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Johnson",
    "email": {
      "value": "morgan.5911@sandbox.example.com"
    },
    "id": "cust_cdf282ef50534baea42c8f163fefbf8d",
    "phone_number": "+918507404977"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "3099 Main Ave"
      },
      "line2": {
        "value": "1496 Pine Blvd"
      },
      "line3": {
        "value": "4010 Market Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83154"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5228@example.com"
      },
      "phone_number": {
        "value": "1676089766"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3878 Main Rd"
      },
      "line2": {
        "value": "4250 Market Ln"
      },
      "line3": {
        "value": "2740 Sunset Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55849"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8542@testmail.io"
      },
      "phone_number": {
        "value": "2488934791"
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
date: Mon, 23 Mar 2026 16:30:59 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

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
    "cf-ray": "9e0ed2e579f547d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:30:59 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=h.S8lod3dSML9twnubckfCBHTTU__IpHEz7CCWCN7eY-1774283459.4356377-1.0.1.1-iXys..e4vmyefMflOpRK.bP_.kwXonGZnpnIC9RzWQWQ3fGLTHVv8bBpMGdEnUz6XFXr3SceWxAHg7JzWArZ9mMCUbyWuqYEhdM.ATUuZBWZtA.IpAne6iwt29ZQjz4h; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:00:59 GMT",
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
  "merchant_capture_id": "mci_ce573fcc42c24de4a29eccab0542322d"
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
date: Mon, 23 Mar 2026 16:30:59 GMT
x-request-id: capture_capture_full_amount_req

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
    "cf-ray": "9e0ed2e68a9747d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:30:59 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=_22o2yUwC0B.4KPObl5dSwEYtBANJflcOsHEQIaDGGQ-1774283459.603493-1.0.1.1-92YvJKLdsNXPsSOt82ZgN0wtxE0hxaLJ.5.Z.pe5qohT1b0cJ9ZWNHzy308JyD4H94M72zTmHEJeRFPGsktSBhLEGNft2gm4E9VyOjxTB6Biqtj7VE1oGcmIjHgA4mow; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:00:59 GMT",
    "strict-transport-security": "max-age=2592000; includeSubDomains"
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
