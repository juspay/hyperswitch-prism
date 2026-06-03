# Connector `stripe` / Suite `PaymentService/Capture` / Scenario `Capture | Merchant Order ID Reference`

- Service: `PaymentService/Capture`
- Scenario Key: `capture_with_merchant_order_id`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. CustomerService/Create(create_customer) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: CustomerService/Create_create_customer_req" \
  -H "x-connector-request-reference-id: CustomerService/Create_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_0bd3c49849594cdf8ce601e3",
  "customer_name": "Noah Brown",
  "email": {
    "value": "jordan.8514@testmail.io"
  },
  "phone_number": "+913866296586",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "7430 Main Ln"
      },
      "line2": {
        "value": "2844 Market St"
      },
      "line3": {
        "value": "7514 Lake Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99714"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3248@testmail.io"
      },
      "phone_number": {
        "value": "6308437788"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2603 Pine Blvd"
      },
      "line2": {
        "value": "6141 Sunset St"
      },
      "line3": {
        "value": "3479 Main Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56203"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8372@sandbox.example.com"
      },
      "phone_number": {
        "value": "4915417495"
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
x-connector-request-reference-id: CustomerService/Create_create_customer_ref
x-merchant-id: test_merchant
x-request-id: CustomerService/Create_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:42 GMT
x-request-id: CustomerService/Create_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UJktzapgm8Q3U0",
  "connectorCustomerId": "cus_UJktzapgm8Q3U0",
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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:39:42 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_m8GGiuZNzAWllU",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_m8GGiuZNzAWllU",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. PaymentMethodService/Tokenize(tokenize_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentMethodService/Tokenize_tokenize_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentMethodService/Tokenize_tokenize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_347244",
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
    "id": "cust_254a38217e4448538b1f390b",
    "name": "Ava Wilson",
    "email": {
      "value": "sam.1834@sandbox.example.com"
    },
    "connector_customer_id": "cus_UJktzapgm8Q3U0"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2603 Pine Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56203"
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
x-connector-request-reference-id: PaymentMethodService/Tokenize_tokenize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentMethodService/Tokenize_tokenize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:42 GMT
x-request-id: PaymentMethodService/Tokenize_tokenize_credit_card_req

Response contents:
{
  "paymentMethodToken": ***MASKED***"
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "1114",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:39:42 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_osPnFHRwhq12vq",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_osPnFHRwhq12vq",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  },
  "merchantPaymentMethodId": "pm_1TL7NaD5R7gDAGff6lr3woNI"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>3. PaymentService/Authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_da68a7d35c28466b9583e528",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
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
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Wilson",
    "email": {
      "value": "sam.1834@sandbox.example.com"
    },
    "id": "cust_254a38217e4448538b1f390b",
    "phone_number": "+443578168572",
    "connector_customer_id": "cus_UJktzapgm8Q3U0"
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "7430 Main Ln"
      },
      "line2": {
        "value": "2844 Market St"
      },
      "line3": {
        "value": "7514 Lake Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99714"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3248@testmail.io"
      },
      "phone_number": {
        "value": "6308437788"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2603 Pine Blvd"
      },
      "line2": {
        "value": "6141 Sunset St"
      },
      "line3": {
        "value": "3479 Main Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56203"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8372@sandbox.example.com"
      },
      "phone_number": {
        "value": "4915417495"
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
  "locale": "en-US",
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
  "order_details": []
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:44 GMT
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TL7NbD5R7gDAGff1M006XxG",
  "connectorTransactionId": "pi_3TL7NbD5R7gDAGff1M006XxG",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "5631",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:39:44 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_lhNL77VyXq7kDD",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_lhNL77VyXq7kDD",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  },
  "networkTransactionId": "104557651805572",
  "state": {
    "connectorCustomerId": "cus_UJktzapgm8Q3U0"
  },
  "rawConnectorResponse": "***MASKED***",
  "rawConnectorRequest": "***MASKED***",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TL7NbD5R7gDAGffJTS4n2pR",
      "paymentMethodId": "pm_1TL7NbD5R7gDAGffJTS4n2pR"
    }
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjoicGFzcyIsImFkZHJlc3NfcG9zdGFsX2NvZGVfY2hlY2siOiJwYXNzIiwiY3ZjX2NoZWNrIjoicGFzcyJ9"
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: PaymentService/Capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "pi_3TL7NbD5R7gDAGff1M006XxG",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_6fa29dd9ac984bc9b4cb7663",
  "merchant_order_id": "gen_414808",
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
  "state": {
    "connector_customer_id": "cus_UJktzapgm8Q3U0"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment by transferring funds. Captures the authorized
// amount to complete the transaction and move funds to your merchant account.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:45 GMT
x-request-id: PaymentService/Capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "pi_3TL7NbD5R7gDAGff1M006XxG",
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
    "content-length": "1884",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H",
    "content-type": "application/json",
    "date": "Sat, 11 Apr 2026 19:39:45 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_H2NB7gGg9RgtqX",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_H2NB7gGg9RgtqX",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "3c3"
  },
  "merchantCaptureId": "pi_3TL7NbD5R7gDAGff1M006XxG",
  "rawConnectorRequest": "***MASKED***",
  "capturedAmount": "6000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TL7NbD5R7gDAGffJTS4n2pR",
      "paymentMethodId": "pm_1TL7NbD5R7gDAGffJTS4n2pR"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentservice-capture.md) | [Back to Overview](../../../test_overview.md)
