# Connector `mollie` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

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
date: Tue, 24 Mar 2026 07:03:40 GMT
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
  "merchant_customer_id": "mcui_c3ab30e1c5b643c784c33f6c",
  "customer_name": "Liam Brown",
  "email": {
    "value": "morgan.5273@testmail.io"
  },
  "phone_number": "+912611233661",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9601 Sunset Ln"
      },
      "line2": {
        "value": "226 Main Dr"
      },
      "line3": {
        "value": "8257 Oak Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21166"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7799@example.com"
      },
      "phone_number": {
        "value": "1375789304"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "4529 Sunset Blvd"
      },
      "line2": {
        "value": "7325 Pine Blvd"
      },
      "line3": {
        "value": "2064 Oak Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63680"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9941@testmail.io"
      },
      "phone_number": {
        "value": "1825824775"
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
date: Tue, 24 Mar 2026 07:03:40 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
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
  "merchant_transaction_id": "mti_2959f649a8d843a7816890e5",
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
        "value": "Emma Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "casey.8873@example.com"
    },
    "id": "cust_e5136ad1a5ab4bf98dc82903",
    "phone_number": "+12514410210"
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
        "value": "Miller"
      },
      "line1": {
        "value": "9601 Sunset Ln"
      },
      "line2": {
        "value": "226 Main Dr"
      },
      "line3": {
        "value": "8257 Oak Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21166"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7799@example.com"
      },
      "phone_number": {
        "value": "1375789304"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "4529 Sunset Blvd"
      },
      "line2": {
        "value": "7325 Pine Blvd"
      },
      "line3": {
        "value": "2064 Oak Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63680"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9941@testmail.io"
      },
      "phone_number": {
        "value": "1825824775"
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
date: Tue, 24 Mar 2026 07:03:41 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "tr_kEATDqiWhNihCzUsRdrNJ",
  "connectorTransactionId": "tr_kEATDqiWhNihCzUsRdrNJ",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "alt-svc": "h3=\":443\"; ma=2592000,h3-29=\":443\"; ma=2592000",
    "content-length": "1063",
    "content-type": "application/hal+json",
    "date": "Tue, 24 Mar 2026 07:03:40 GMT",
    "server": "Mollie",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "via": "1.1 google, 1.1 google",
    "x-mollie-api-gateway-requestid": "019d1ea803e676f5a1713276175c00f6",
    "x-robots-tag": "noindex",
    "x-xss-protection": "1; mode=block"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://www.mollie.com/checkout/credit-card/session/kEATDqiWhNihCzUsRdrNJ",
      "method": "HTTP_METHOD_GET"
    }
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
