# Connector `stripe` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

- Service: `PaymentService/SetupRecurring`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_ee1c942ac03849b5ae09d08765a88e3f",
  "customer_name": "Noah Johnson",
  "email": {
    "value": "riley.4754@example.com"
  },
  "phone_number": "+915280169314",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "6262 Main Ln"
      },
      "line2": {
        "value": "9533 Main Blvd"
      },
      "line3": {
        "value": "4618 Market Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "26592"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2764@sandbox.example.com"
      },
      "phone_number": {
        "value": "8169624083"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8573 Lake Blvd"
      },
      "line2": {
        "value": "4127 Oak St"
      },
      "line3": {
        "value": "3990 Pine Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31290"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.7477@testmail.io"
      },
      "phone_number": {
        "value": "1335145265"
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
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: stripe
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:27 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8gvz28mLNq1hK",
  "connectorCustomerId": "cus_U8gvz28mLNq1hK",
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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:27 GMT",
    "idempotency-key": "933a05e2-fd33-4d46-af8c-1bfe93752170",
    "original-request": "req_qKMZJCxP7ZZEA8",
    "request-id": "req_qKMZJCxP7ZZEA8",
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
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_with_webhook_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_7e4cc01a48744ba688650822599591a9",
  "amount": {
    "minor_amount": 4500,
    "currency": "USD"
  },
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
        "value": "Ava Brown"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "alex.1634@example.com"
    },
    "id": "cust_34905c321c1d4382aa93de6da08764d2",
    "phone_number": "+19008000069",
    "connector_customer_id": "cus_U8gvz28mLNq1hK"
  },
  "webhook_url": "https://example.com/payment/webhook",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8573 Lake Blvd"
      },
      "line2": {
        "value": "4127 Oak St"
      },
      "line3": {
        "value": "3990 Pine Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31290"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.7477@testmail.io"
      },
      "phone_number": {
        "value": "1335145265"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION",
  "return_url": "https://example.com/payment/return"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: stripe
x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_with_webhook_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:31 GMT
x-request-id: setup_recurring_setup_recurring_with_webhook_req

Response contents:
{
  "connectorRecurringPaymentId": "seti_1TAPXKD5R7gDAGffBd4NqWG4",
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
    "content-length": "1997",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:31 GMT",
    "idempotency-key": "3ea4f79d-08f9-47f5-930d-a981ee368f75",
    "original-request": "req_emHXecsV03BmMx",
    "request-id": "req_emHXecsV03BmMx",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPXKD5R7gDAGffZvp6pIhW",
      "paymentMethodId": "pm_1TAPXKD5R7gDAGffZvp6pIhW"
    }
  },
  "merchantRecurringPaymentId": "seti_1TAPXKD5R7gDAGffBd4NqWG4",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjpudWxsLCJhZGRyZXNzX3Bvc3RhbF9jb2RlX2NoZWNrIjpudWxsLCJjdmNfY2hlY2siOiJwYXNzIn0="
      }
    }
  },
  "capturedAmount": "0",
  "state": {
    "connectorCustomerId": "cus_U8gvz28mLNq1hK"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/setup_intents\",\"method\":\"POST\",\"headers\":{\"stripe-version\":\"2022-11-15\",\"Content-Type\":\"application/x-www-form-urlencoded\",\"Authorization\":\"Bearer ***MASKED***",\"via\":\"HyperSwitch\"},\"body\":\"confirm=true\u0026usage=off_session\u0026customer=cus_U8gvz28mLNq1hK\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026metadata%5Border_id%5D=mrpi_7e4cc01a48744ba688650822599591a9\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_attempt\"}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
