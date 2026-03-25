# Connector `stripe` / Suite `tokenize_payment_method` / Scenario `tokenize_with_metadata`

- Service: `Unknown`
- PM / PMT: `card` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"parameter_unknown","message":"Received unknown parameter: type","reason":"Received unknown parameter: type"}}
```

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
  "merchant_customer_id": "mcui_67e6e0b102b146468d7308c4",
  "customer_name": "Mia Miller",
  "email": {
    "value": "jordan.5123@testmail.io"
  },
  "phone_number": "+914070685946",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7512 Market Dr"
      },
      "line2": {
        "value": "3656 Oak Blvd"
      },
      "line3": {
        "value": "6630 Oak Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52204"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.4200@example.com"
      },
      "phone_number": {
        "value": "4566054402"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8812 Sunset Ave"
      },
      "line2": {
        "value": "3315 Pine Ln"
      },
      "line3": {
        "value": "9698 Pine Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99127"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1445@example.com"
      },
      "phone_number": {
        "value": "4184243421"
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
date: Tue, 24 Mar 2026 08:50:22 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCqMe4e5qvR6x1",
  "connectorCustomerId": "cus_UCqMe4e5qvR6x1",
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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:50:22 GMT",
    "idempotency-key": "393cf831-eea5-464c-9875-95c775ffd7b6",
    "original-request": "req_fimxkFuzPw4v7N",
    "request-id": "req_fimxkFuzPw4v7N",
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
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_with_metadata_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_with_metadata_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_951553",
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
        "value": "08"
      },
      "card_exp_year": {
        "value": "2029"
      },
      "card_cvc": ***MASKED***
        "value": "789"
      },
      "card_holder_name": {
        "value": "Test User"
      }
    }
  },
  "customer": {
    "id": "cust_35668ff6d7b141288ea50e34",
    "name": "Emma Miller",
    "email": {
      "value": "jordan.2204@sandbox.example.com"
    },
    "connector_customer_id": "cus_UCqMe4e5qvR6x1"
  },
  "metadata": {
    "value": "{\"source\":\"mobile\",\"device_id\":\"test-device-123\"}"
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_with_metadata_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_with_metadata_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:50:26 GMT
x-request-id: tokenize_payment_method_tokenize_with_metadata_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "parameter_unknown",
      "message": "Received unknown parameter: type",
      "reason": "Received unknown parameter: type"
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
    "content-length": "357",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:50:26 GMT",
    "idempotency-key": "1a7e6f02-22b1-44a6-b46d-3c9e7ed66df8",
    "original-request": "req_uI9RPfCVVhE8Yv",
    "request-id": "req_uI9RPfCVVhE8Yv",
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


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)
