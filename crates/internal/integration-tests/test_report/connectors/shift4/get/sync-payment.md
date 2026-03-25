# Connector `shift4` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"NO_ERROR_CODE","message":"Provided API key is invalid"}}
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
date: Mon, 23 Mar 2026 16:31:01 GMT
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
  "merchant_customer_id": "mcui_fa208e51766645288d101dd325859578",
  "customer_name": "Ava Smith",
  "email": {
    "value": "sam.9085@example.com"
  },
  "phone_number": "+914160918364",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "5805 Main Blvd"
      },
      "line2": {
        "value": "6621 Pine Rd"
      },
      "line3": {
        "value": "3960 Sunset Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "29895"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1605@sandbox.example.com"
      },
      "phone_number": {
        "value": "2483067432"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5498 Pine Dr"
      },
      "line2": {
        "value": "6177 Sunset Ave"
      },
      "line3": {
        "value": "119 Market Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "13335"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1807@testmail.io"
      },
      "phone_number": {
        "value": "9423345396"
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
date: Mon, 23 Mar 2026 16:31:01 GMT
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
  "merchant_transaction_id": "mti_21870efa52414bb8b1507159386b4c7d",
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
        "value": "Mia Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "alex.7132@example.com"
    },
    "id": "cust_e92bd32c20d043698008d5331c4cc0f4",
    "phone_number": "+918870671628"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "5805 Main Blvd"
      },
      "line2": {
        "value": "6621 Pine Rd"
      },
      "line3": {
        "value": "3960 Sunset Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "29895"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1605@sandbox.example.com"
      },
      "phone_number": {
        "value": "2483067432"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5498 Pine Dr"
      },
      "line2": {
        "value": "6177 Sunset Ave"
      },
      "line3": {
        "value": "119 Market Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "13335"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1807@testmail.io"
      },
      "phone_number": {
        "value": "9423345396"
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
date: Mon, 23 Mar 2026 16:31:02 GMT
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
    "cf-ray": "9e0ed2f53b7d47d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:31:01 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=GTjJ1VOPHn4jOL3y1CBbo21g6_5F9FsCGsuu8e_TyCk-1774283461.9525425-1.0.1.1-wlzx59DiB90NzTICOqgt8JlWyEnsWSvJtPfY01kEAnvhCcCuqbi29ihW8JoE2.uqyksgybgN._FIN1W.pnBOsFDB2dCX8Lc.KKyDcMPsQDB7s.aMfYAev2OC3TMQenJW; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:01:01 GMT",
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
  -H "x-request-id: get_sync_payment_req" \
  -H "x-connector-request-reference-id: get_sync_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount": {
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
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:31:02 GMT
x-request-id: get_sync_payment_req

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
    "cf-ray": "9e0ed2f64c3a47d0-BOM",
    "connection": "keep-alive",
    "content-length": "99",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:31:02 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=4VfL7WBwSyXO6.RNnjeS34ULdKcX8BSoP5Vhup_3yvE-1774283462.1305945-1.0.1.1-bSxxcgjUYX9WVWmhhXMeEoA_Vqq1HlLhhefsRyz2UMQplpUFmXaNZ3YGOqLv7v5tDgDu7_La2kWgFNiv15EYHOs21eY5zUkd4oCiCg4.p5wd6gnm8Drrj38.8OLEnGlw; HttpOnly; Secure; Path=/; Domain=api.shift4.com; Expires=Mon, 23 Mar 2026 17:01:02 GMT",
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


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
