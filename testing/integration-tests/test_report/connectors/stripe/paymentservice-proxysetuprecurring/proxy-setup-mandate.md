# Connector `stripe` / Suite `PaymentService/ProxySetupRecurring` / Scenario `Proxy Payment | Setup Mandate`

- Service: `Unknown`
- Scenario Key: `proxy_setup_mandate`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Setup recurring mandate using vault-aliased card data.
rpc ProxySetupRecurring ( .types.PaymentServiceProxySetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/ProxySetupRecurring_proxy_setup_mandate_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/ProxySetupRecurring_proxy_setup_mandate_req
x-tenant-id: default

Error invoking method "types.PaymentService/ProxySetupRecurring": error getting request data: message type types.MandateType has no known field named mandate_type
```

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
  "merchant_customer_id": "mcui_9f1001a7bfc64a3fa38d78cf",
  "customer_name": "Noah Johnson",
  "email": {
    "value": "alex.5928@example.com"
  },
  "phone_number": "+915139790843",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2594 Sunset St"
      },
      "line2": {
        "value": "3527 Main St"
      },
      "line3": {
        "value": "9059 Oak Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34977"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1907@example.com"
      },
      "phone_number": {
        "value": "9365225216"
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
        "value": "3996 Sunset St"
      },
      "line2": {
        "value": "8329 Pine Ave"
      },
      "line3": {
        "value": "7503 Pine Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "94447"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5826@example.com"
      },
      "phone_number": {
        "value": "5007703239"
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
date: Sat, 11 Apr 2026 19:40:01 GMT
x-request-id: CustomerService/Create_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UJktq6Wj7AFJZp",
  "connectorCustomerId": "cus_UJktq6Wj7AFJZp",
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
    "date": "Sat, 11 Apr 2026 19:40:01 GMT",
    "idempotency-key": ***MASKED***"
    "original-request": "req_ESQibgcL3HSEX7",
    "report-to": "{\"group\":\"csp\",\"max_age\":8640,\"endpoints\":[{\"url\":\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"}],\"include_subdomains\":true}",
    "reporting-endpoints": "csp=\"https://q.stripe.com/csp-report-v2?q=JaMLJqKnw1CTgH7YnzaVo8U_uyZOcUzREzca49DgSuTQxXcyJ-yb-t4rqpzMbTRZVMFBmIOHWHC6gV0H\u0026t=1\"",
    "request-id": "req_ESQibgcL3HSEX7",
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
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/ProxySetupRecurring_proxy_setup_mandate_req" \
  -H "x-connector-request-reference-id: PaymentService/ProxySetupRecurring_proxy_setup_mandate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/ProxySetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_a7d310ec6059404dae682f72",
  "amount": {
    "minor_amount": 0,
    "currency": "USD"
  },
  "card_proxy": {
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
    "card_type": "credit"
  },
  "customer": {
    "id": "cust_ce647863763b4e50a4b897d2"
  },
  "setup_mandate_details": {
    "mandate_type": {
      "mandate_type": {
        "MultiUse": {
          "amount": 10000,
          "currency": "USD"
        }
      }
    }
  },
  "customer_acceptance": {
    "acceptance_type": "ONLINE",
    "accepted_at": 1704067200
  },
  "auth_type": "NO_THREE_DS"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Setup recurring mandate using vault-aliased card data.
rpc ProxySetupRecurring ( .types.PaymentServiceProxySetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/ProxySetupRecurring_proxy_setup_mandate_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/ProxySetupRecurring_proxy_setup_mandate_req
x-tenant-id: default

Error invoking method "types.PaymentService/ProxySetupRecurring": error getting request data: message type types.MandateType has no known field named mandate_type
```

</details>


[Back to Connector Suite](../paymentservice-proxysetuprecurring.md) | [Back to Overview](../../../test_overview.md)
