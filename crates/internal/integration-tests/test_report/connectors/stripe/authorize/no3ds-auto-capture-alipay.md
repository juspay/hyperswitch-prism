# Connector `stripe` / Suite `authorize` / Scenario `Alipay | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_alipay`
- PM / PMT: `ali_pay_redirect` / `-`
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
  "merchant_customer_id": "mcui_1d0efd5081a54d15a47ce8e30d931feb",
  "customer_name": "Liam Taylor",
  "email": {
    "value": "morgan.7604@testmail.io"
  },
  "phone_number": "+15097358998",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6047 Oak Dr"
      },
      "line2": {
        "value": "4207 Market Dr"
      },
      "line3": {
        "value": "329 Sunset Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "92568"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2682@sandbox.example.com"
      },
      "phone_number": {
        "value": "6958324857"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "7263 Market Rd"
      },
      "line2": {
        "value": "2345 Lake Ave"
      },
      "line3": {
        "value": "7227 Pine Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27158"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8429@example.com"
      },
      "phone_number": {
        "value": "8195830751"
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
date: Mon, 23 Mar 2026 15:46:21 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCZqXfe9voW9zB",
  "connectorCustomerId": "cus_UCZqXfe9voW9zB",
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
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:46:21 GMT",
    "idempotency-key": "f9e942b9-5ac3-4cc6-a6da-5e833399c911",
    "original-request": "req_5ZSSFBWt0L0Fvk",
    "request-id": "req_5ZSSFBWt0L0Fvk",
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
  -H "x-request-id: authorize_no3ds_auto_capture_alipay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_alipay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_070921d5ab4341b299be31d8d7dbb2e4",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "ali_pay_redirect": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Johnson",
    "email": {
      "value": "jordan.3137@sandbox.example.com"
    },
    "id": "cust_105a55b9f0f8463385fd6512b9c101ad",
    "phone_number": "+447348363884",
    "connector_customer_id": "cus_UCZqXfe9voW9zB"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6047 Oak Dr"
      },
      "line2": {
        "value": "4207 Market Dr"
      },
      "line3": {
        "value": "329 Sunset Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "92568"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2682@sandbox.example.com"
      },
      "phone_number": {
        "value": "6958324857"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "7263 Market Rd"
      },
      "line2": {
        "value": "2345 Lake Ave"
      },
      "line3": {
        "value": "7227 Pine Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27158"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8429@example.com"
      },
      "phone_number": {
        "value": "8195830751"
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
    "user_agent": "Mozilla/5.0 (ucs-connector-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  },
  "off_session": false,
  "description": "No3DS auto capture Alipay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_alipay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_alipay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:46:27 GMT
x-request-id: authorize_no3ds_auto_capture_alipay_req

Response contents:
{
  "merchantTransactionId": "pi_3TEAgRD5R7gDAGff1aQlMw1Q",
  "connectorTransactionId": "pi_3TEAgRD5R7gDAGff1aQlMw1Q",
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
    "content-length": "2014",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:46:27 GMT",
    "idempotency-key": "9ce53234-4ec4-4724-82af-f9498e894926",
    "original-request": "req_T1WIWk50hvuAFZ",
    "request-id": "req_T1WIWk50hvuAFZ",
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
      "endpoint": "https://hooks.stripe.com/redirect/authenticate/src_1TEAgRD5R7gDAGffqQzs4kh8",
      "method": "HTTP_METHOD_GET",
      "formFields": {
        "client_secret": ***MASKED***"
      }
    }
  },
  "state": {
    "connectorCustomerId": "cus_UCZqXfe9voW9zB"
  },
  "rawConnectorResponse": "***MASKED***"
  
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEAgRD5R7gDAGffNaTBOGPb",
      "paymentMethodId": "pm_1TEAgRD5R7gDAGffNaTBOGPb"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)