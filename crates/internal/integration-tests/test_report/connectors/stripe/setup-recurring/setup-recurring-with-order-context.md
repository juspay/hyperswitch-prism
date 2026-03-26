# Connector `stripe` / Suite `setup_recurring` / Scenario `Setup Recurring | Order Context`

- Service: `PaymentService/SetupRecurring`
- Scenario Key: `setup_recurring_with_order_context`
- PM / PMT: `card` / `credit`
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
  "merchant_customer_id": "mcui_aa94336be5224b689bc9af13bb3f08f2",
  "customer_name": "Liam Brown",
  "email": {
    "value": "alex.6190@example.com"
  },
  "phone_number": "+918822894537",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "2955 Main Dr"
      },
      "line2": {
        "value": "474 Pine Dr"
      },
      "line3": {
        "value": "1714 Lake Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "72015"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3577@sandbox.example.com"
      },
      "phone_number": {
        "value": "3758710125"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6414 Lake St"
      },
      "line2": {
        "value": "5159 Market Blvd"
      },
      "line3": {
        "value": "6363 Sunset St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42461"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9004@sandbox.example.com"
      },
      "phone_number": {
        "value": "5453378723"
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
date: Mon, 23 Mar 2026 15:48:04 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCZrDgb8v8MGm0",
  "connectorCustomerId": "cus_UCZrDgb8v8MGm0",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "669",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:04 GMT",
    "idempotency-key": "b3b3a866-98fc-4cc8-b59b-f7fa55451aee",
    "original-request": "req_2DETja8cbxncJI",
    "request-id": "req_2DETja8cbxncJI",
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
  -H "x-request-id: setup_recurring_setup_recurring_with_order_context_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_with_order_context_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_3f53e1f3807e4075a8f36f891a1f6470",
  "amount": {
    "minor_amount": 6000,
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
        "value": "Noah Taylor"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ava Brown",
    "email": {
      "value": "alex.9929@sandbox.example.com"
    },
    "id": "cust_83c38b10a0484c5a86232cd27a7b3d36",
    "phone_number": "+911571578987",
    "connector_customer_id": "cus_UCZrDgb8v8MGm0"
  },
  "complete_authorize_url": "https://example.com/payment/complete",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6414 Lake St"
      },
      "line2": {
        "value": "5159 Market Blvd"
      },
      "line3": {
        "value": "6363 Sunset St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42461"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9004@sandbox.example.com"
      },
      "phone_number": {
        "value": "5453378723"
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
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (ucs-connector-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  },
  "merchant_order_id": "gen_114942",
  "order_category": "subscription",
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook"
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
x-connector-config: ***MASKED***
x-connector-request-reference-id: setup_recurring_setup_recurring_with_order_context_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_with_order_context_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:07 GMT
x-request-id: setup_recurring_setup_recurring_with_order_context_req

Response contents:
{
  "connectorRecurringPaymentId": "seti_1TEAi2D5R7gDAGfff2WlgUR2",
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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:07 GMT",
    "idempotency-key": "fa1c712c-4394-45cd-a1b1-4ef257bb170c",
    "original-request": "req_Jr87w4lXfxV7mf",
    "request-id": "req_Jr87w4lXfxV7mf",
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
      "connectorMandateId": "pm_1TEAi2D5R7gDAGff92tuv1hR",
      "paymentMethodId": "pm_1TEAi2D5R7gDAGff92tuv1hR"
    }
  },
  "merchantRecurringPaymentId": "seti_1TEAi2D5R7gDAGfff2WlgUR2",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjpudWxsLCJhZGRyZXNzX3Bvc3RhbF9jb2RlX2NoZWNrIjpudWxsLCJjdmNfY2hlY2siOiJwYXNzIn0="
      }
    }
  },
  "capturedAmount": "6000",
  "state": {
    "connectorCustomerId": "cus_UCZrDgb8v8MGm0"
  },
  "rawConnectorRequest": "***MASKED***"
  
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)