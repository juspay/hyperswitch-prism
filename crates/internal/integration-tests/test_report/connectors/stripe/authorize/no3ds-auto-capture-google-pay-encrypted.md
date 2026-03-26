# Connector `stripe` / Suite `authorize` / Scenario `Google Pay (Encrypted Token) | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_google_pay_encrypted`
- PM / PMT: `google_pay` / `CARD`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — PASS</summary>

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
  "merchant_customer_id": "mcui_f22853884f2e4b658bf0512d",
  "customer_name": "Mia Taylor",
  "email": {
    "value": "morgan.1476@testmail.io"
  },
  "phone_number": "+17946541249",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4348 Oak St"
      },
      "line2": {
        "value": "6681 Sunset St"
      },
      "line3": {
        "value": "9828 Oak Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12820"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9718@example.com"
      },
      "phone_number": {
        "value": "9358479840"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3302 Sunset Ave"
      },
      "line2": {
        "value": "8566 Main St"
      },
      "line3": {
        "value": "845 Pine Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64295"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9754@testmail.io"
      },
      "phone_number": {
        "value": "8378221878"
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
content-type: application/grpc
date: Wed, 25 Mar 2026 13:02:56 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UDHeYKvSlzITee",
  "connectorCustomerId": "cus_UDHeYKvSlzITee",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "671",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=6vN0g-AvWWjnR-b8XN2mTaRJIZ2J9FcH9LGUmljrm696AJYys6d3JswRz2UEAMHD2VnKC-v1I13raGr_",
    "content-type": "application/json",
    "date": "Wed, 25 Mar 2026 13:02:56 GMT",
    "idempotency-key": "e6cb4b56-f41f-449e-8f87-0a40a6d63e05",
    "original-request": "req_morF69Wzwgk0xo",
    "request-id": "req_morF69Wzwgk0xo",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. tokenize_payment_method(tokenize_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"parameter_unknown","message":"Received unknown parameters: billing_details, type","reason":"Received unknown parameters: billing_details, type"}}
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_credit_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_388033",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "John Doe"
      }
    }
  },
  "customer": {
    "id": "cust_cd2249bfd55e40a49f4688f1",
    "name": "Ethan Johnson",
    "email": {
      "value": "sam.3994@sandbox.example.com"
    },
    "connector_customer_id": "cus_UDHeYKvSlzITee"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3302 Sunset Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64295"
      },
      "country_alpha2_code": "US"
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
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Wed, 25 Mar 2026 13:02:57 GMT
x-request-id: tokenize_payment_method_tokenize_credit_card_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "parameter_unknown",
      "message": "Received unknown parameters: billing_details, type",
      "reason": "Received unknown parameters: billing_details, type"
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "386",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=6vN0g-AvWWjnR-b8XN2mTaRJIZ2J9FcH9LGUmljrm696AJYys6d3JswRz2UEAMHD2VnKC-v1I13raGr_",
    "content-type": "application/json",
    "date": "Wed, 25 Mar 2026 13:02:57 GMT",
    "idempotency-key": "97651f1b-6936-489e-aa1c-db51a0b9c0fe",
    "original-request": "req_i9dIVg3bNgZ9vv",
    "request-id": "req_i9dIVg3bNgZ9vv",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
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
  -H "x-request-id: authorize_no3ds_auto_capture_google_pay_encrypted_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_google_pay_encrypted_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_ec08c3ba3e2d48c8ad46665b",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "google_pay": {
      "type": "CARD",
      "description": "Visa 1111",
      "info": {
        "card_network": "VISA",
        "card_details": "1111"
      },
      "tokenization_data": ***MASKED***
        "encrypted_data": {
          "token": ***MASKED***"
          "token_type": ***MASKED***"
        }
      }
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "sam.3994@sandbox.example.com"
    },
    "id": "cust_cd2249bfd55e40a49f4688f1",
    "phone_number": "+17138314400",
    "connector_customer_id": "cus_UDHeYKvSlzITee"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "4348 Oak St"
      },
      "line2": {
        "value": "6681 Sunset St"
      },
      "line3": {
        "value": "9828 Oak Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "12820"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9718@example.com"
      },
      "phone_number": {
        "value": "9358479840"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3302 Sunset Ave"
      },
      "line2": {
        "value": "8566 Main St"
      },
      "line3": {
        "value": "845 Pine Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64295"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9754@testmail.io"
      },
      "phone_number": {
        "value": "8378221878"
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
  "description": "No3DS auto capture Google Pay (encrypted token)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_google_pay_encrypted_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_google_pay_encrypted_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Wed, 25 Mar 2026 13:03:27 GMT
x-request-id: authorize_no3ds_auto_capture_google_pay_encrypted_req

Response contents:
{
  "merchantTransactionId": "pi_3TEr5lD5R7gDAGff1rIGTWml",
  "connectorTransactionId": "pi_3TEr5lD5R7gDAGff1rIGTWml",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "5639",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=6vN0g-AvWWjnR-b8XN2mTaRJIZ2J9FcH9LGUmljrm696AJYys6d3JswRz2UEAMHD2VnKC-v1I13raGr_",
    "content-type": "application/json",
    "date": "Wed, 25 Mar 2026 13:03:26 GMT",
    "idempotency-key": "f1cc3edc-efff-4128-a647-a8fce59936ad",
    "original-request": "req_FWmViqHod9xBjx",
    "request-id": "req_FWmViqHod9xBjx",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "networkTransactionId": "104557651805572",
  "state": {
    "connectorCustomerId": "cus_UDHeYKvSlzITee"
  },
  "rawConnectorResponse": "***MASKED***"
  
  "capturedAmount": "6000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEr5lD5R7gDAGffmXtSQDJq",
      "paymentMethodId": "pm_1TEr5lD5R7gDAGffmXtSQDJq"
    }
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjoicGFzcyIsImFkZHJlc3NfcG9zdGFsX2NvZGVfY2hlY2siOiJwYXNzIiwiY3ZjX2NoZWNrIjpudWxsfQ=="
      }
    },
    "extendedAuthorizationResponseData": ***MASKED***
      "extendedAuthenticationApplied": false
    },
    "isOvercaptureEnabled": false
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)