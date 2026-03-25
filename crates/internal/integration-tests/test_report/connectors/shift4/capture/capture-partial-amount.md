# Connector `shift4` / Suite `capture` / Scenario `capture_partial_amount`

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
  "merchant_customer_id": "mcui_b94a6039a17b4f069c13a5613a9f52eb",
  "customer_name": "Ava Brown",
  "email": {
    "value": "jordan.3479@testmail.io"
  },
  "phone_number": "+16777812866",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9165 Lake Ave"
      },
      "line2": {
        "value": "4257 Lake Ln"
      },
      "line3": {
        "value": "4775 Lake Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44464"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1352@testmail.io"
      },
      "phone_number": {
        "value": "9233444944"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "6589 Sunset Dr"
      },
      "line2": {
        "value": "687 Main Ln"
      },
      "line3": {
        "value": "2205 Oak Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70916"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9542@example.com"
      },
      "phone_number": {
        "value": "4961403909"
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
  "merchant_transaction_id": "mti_27303bc7855745f2892a95b9e9d34cce",
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
        "value": "Noah Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Smith",
    "email": {
      "value": "riley.1725@sandbox.example.com"
    },
    "id": "cust_4aab9f37fcd64bd799aa5ed98fd32f81",
    "phone_number": "+441782208370"
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
        "value": "9165 Lake Ave"
      },
      "line2": {
        "value": "4257 Lake Ln"
      },
      "line3": {
        "value": "4775 Lake Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44464"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1352@testmail.io"
      },
      "phone_number": {
        "value": "9233444944"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "6589 Sunset Dr"
      },
      "line2": {
        "value": "687 Main Ln"
      },
      "line3": {
        "value": "2205 Oak Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70916"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9542@example.com"
      },
      "phone_number": {
        "value": "4961403909"
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
date: Mon, 23 Mar 2026 16:31:00 GMT
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
    "cf-ray": "9e0ed2e93c0347d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:31:00 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=4jX8RuQpZr_3Nj3OGXB_2khwmHWcluDejiUDL9pfEWs-1774283460.0395887-1.0.1.1-PG17gq7DQmu2N1PTB2iIKPxX4UBaxnaUVCpKJSapz2irL66dl6p3cpR8pmFmB6R_1msnr30vxRpocpFiYLhRXmPElIk3GLl5ej_DtmcAEvIwcYaZxOptaGXiPzowNq_9; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:01:00 GMT",
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
  -H "x-request-id: capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_38ae9fdd4e6b4a94b2067e7364c1ea16"
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
x-connector-request-reference-id: capture_capture_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:31:00 GMT
x-request-id: capture_capture_partial_amount_req

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
    "cf-ray": "9e0ed2eadd1f47d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:31:00 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=h69RfsDtIWs0nebM4WBppOdGZ4XScrzqVxE0QuoOJAY-1774283460.2914932-1.0.1.1-DwNk31B3pEl9UJKcqG7ksVAUDo7CJQKIVIc2KJNegLJj20rS.iU8taolJjhczkmrm1e0AQ00TSZVXE6DutNrg02vIHyz6rNyCA25qjDmYOcF020LkcgeCyCQ_YG28oaG; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:01:00 GMT",
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
