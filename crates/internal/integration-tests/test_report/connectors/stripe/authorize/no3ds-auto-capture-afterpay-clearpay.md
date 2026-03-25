# Connector `stripe` / Suite `authorize` / Scenario `no3ds_auto_capture_afterpay_clearpay`

- Service: `PaymentService/Authorize`
- PM / PMT: `afterpay_clearpay` / `-`
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
  "merchant_customer_id": "mcui_6f59585fc6854708927c75ad",
  "customer_name": "Ethan Brown",
  "email": {
    "value": "jordan.6593@testmail.io"
  },
  "phone_number": "+16622659604",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2859 Market Blvd"
      },
      "line2": {
        "value": "8689 Market Ave"
      },
      "line3": {
        "value": "5252 Pine Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "46761"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6208@example.com"
      },
      "phone_number": {
        "value": "8495357706"
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
        "value": "4247 Oak Dr"
      },
      "line2": {
        "value": "1120 Lake Ln"
      },
      "line3": {
        "value": "9813 Main Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53737"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4846@sandbox.example.com"
      },
      "phone_number": {
        "value": "5856773928"
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
date: Tue, 24 Mar 2026 08:48:19 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCqJqPLabUddGv",
  "connectorCustomerId": "cus_UCqJqPLabUddGv",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "672",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:48:19 GMT",
    "idempotency-key": "705980ca-06f1-432d-a67a-028c9dbe3747",
    "original-request": "req_uhADtlJl6CV6D9",
    "request-id": "req_uhADtlJl6CV6D9",
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
  "merchant_payment_method_id": "gen_180435",
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
    "id": "cust_0f01f36bba9d457ebda5565c",
    "name": "Noah Taylor",
    "email": {
      "value": "sam.3777@sandbox.example.com"
    },
    "connector_customer_id": "cus_UCqJqPLabUddGv"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4247 Oak Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53737"
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
date: Tue, 24 Mar 2026 08:48:19 GMT
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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:48:19 GMT",
    "idempotency-key": "b3e3ccf7-b3a4-4a5a-9cd3-3f934e1211cc",
    "original-request": "req_H3DD0Ao7BVyUHZ",
    "request-id": "req_H3DD0Ao7BVyUHZ",
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
  -H "x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c3e6d157a585441291c073db",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "afterpay_clearpay": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Taylor",
    "email": {
      "value": "sam.3777@sandbox.example.com"
    },
    "id": "cust_0f01f36bba9d457ebda5565c",
    "phone_number": "+12688880811",
    "connector_customer_id": "cus_UCqJqPLabUddGv"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2859 Market Blvd"
      },
      "line2": {
        "value": "8689 Market Ave"
      },
      "line3": {
        "value": "5252 Pine Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "46761"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6208@example.com"
      },
      "phone_number": {
        "value": "8495357706"
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
        "value": "4247 Oak Dr"
      },
      "line2": {
        "value": "1120 Lake Ln"
      },
      "line3": {
        "value": "9813 Main Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53737"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4846@sandbox.example.com"
      },
      "phone_number": {
        "value": "5856773928"
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
  "off_session": false,
  "description": "No3DS auto capture Afterpay/Clearpay payment",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:48:20 GMT
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req

Response contents:
{
  "merchantTransactionId": "pi_3TEQdMD5R7gDAGff0J7kqCDD",
  "connectorTransactionId": "pi_3TEQdMD5R7gDAGff0J7kqCDD",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "1995",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:48:20 GMT",
    "idempotency-key": "1a6afb96-1348-49a4-878f-b3ca77c39c0c",
    "original-request": "req_63E4JGLMqjgHkX",
    "request-id": "req_63E4JGLMqjgHkX",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://hooks.stripe.com/afterpay_clearpay/acct_1M7fTaD5R7gDAGff/pa_nonce_UCqKAt4yAuDNHvb1hVIKx0zYzJ63kPM/redirect",
      "method": "HTTP_METHOD_GET"
    }
  },
  "state": {
    "connectorCustomerId": "cus_UCqJqPLabUddGv"
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEQdMD5R7gDAGffdb3DAIBz",
      "paymentMethodId": "pm_1TEQdMD5R7gDAGffdb3DAIBz"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
