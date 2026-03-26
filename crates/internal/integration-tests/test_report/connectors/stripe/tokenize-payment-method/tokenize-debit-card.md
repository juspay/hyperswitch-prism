# Connector `stripe` / Suite `tokenize_payment_method` / Scenario `Tokenize Payment Method | Tokenize Debit Card`

- Service: `Unknown`
- Scenario Key: `tokenize_debit_card`
- PM / PMT: `card` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"parameter_unknown","message":"Received unknown parameters: billing_details, type","reason":"Received unknown parameters: billing_details, type"}}
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
  "merchant_customer_id": "mcui_0ab734fdd1df4f3aa0e9c2f269e1370a",
  "customer_name": "Liam Smith",
  "email": {
    "value": "casey.8863@testmail.io"
  },
  "phone_number": "+16191398836",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3668 Main Blvd"
      },
      "line2": {
        "value": "9793 Oak Dr"
      },
      "line3": {
        "value": "9039 Sunset Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53853"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5052@example.com"
      },
      "phone_number": {
        "value": "9109071881"
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
        "value": "1622 Sunset St"
      },
      "line2": {
        "value": "4740 Lake Ave"
      },
      "line3": {
        "value": "9886 Pine St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "47240"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.3291@testmail.io"
      },
      "phone_number": {
        "value": "5006567269"
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
date: Mon, 23 Mar 2026 15:48:18 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCZrwNEmr5baY6",
  "connectorCustomerId": "cus_UCZrwNEmr5baY6",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "670",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:18 GMT",
    "idempotency-key": "57e54152-4ef4-46f1-9ce4-675dc3c67917",
    "original-request": "req_pPL9Su0Al2enny",
    "request-id": "req_pPL9Su0Al2enny",
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
  -H "x-request-id: tokenize_payment_method_tokenize_debit_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_185078",
  "amount": {
    "minor_amount": 0,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "5555555555554444"
      },
      "card_exp_month": {
        "value": "10"
      },
      "card_exp_year": {
        "value": "2028"
      },
      "card_cvc": ***MASKED***
        "value": "456"
      },
      "card_holder_name": {
        "value": "Jane Smith"
      }
    }
  },
  "customer": {
    "id": "cust_0c9ba940e568443c927479027ab36c6d",
    "name": "Noah Johnson",
    "email": {
      "value": "riley.2671@sandbox.example.com"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "1622 Sunset St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "47240"
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
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_debit_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:19 GMT
x-request-id: tokenize_payment_method_tokenize_debit_card_req

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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:19 GMT",
    "idempotency-key": "440708b6-ee4c-4560-9aab-3636005178cd",
    "original-request": "req_l9nqsZAAjGtcPt",
    "request-id": "req_l9nqsZAAjGtcPt",
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