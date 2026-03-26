# Connector `stripe` / Suite `refund_sync` / Scenario `Refund Sync | Reason`

- Service: `RefundService/Get`
- Scenario Key: `refund_sync_with_reason`
- PM / PMT: `-` / `-`
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
  "merchant_customer_id": "mcui_39d00c4e43994f45926673171b3b5a0d",
  "customer_name": "Ethan Smith",
  "email": {
    "value": "alex.2135@sandbox.example.com"
  },
  "phone_number": "+446279715515",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "1231 Oak St"
      },
      "line2": {
        "value": "429 Oak Dr"
      },
      "line3": {
        "value": "7396 Oak Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33270"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4833@sandbox.example.com"
      },
      "phone_number": {
        "value": "6468420307"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9811 Market Blvd"
      },
      "line2": {
        "value": "1754 Oak St"
      },
      "line3": {
        "value": "6444 Main Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "17801"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.5076@example.com"
      },
      "phone_number": {
        "value": "2490427527"
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
date: Mon, 23 Mar 2026 15:47:59 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_UCZrJYMJG3QxmC",
  "connectorCustomerId": "cus_UCZrJYMJG3QxmC",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "678",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:47:59 GMT",
    "idempotency-key": "bca58be8-dacb-41af-8a40-b69d7736b02b",
    "original-request": "req_NfioVQGnIYNSvE",
    "request-id": "req_NfioVQGnIYNSvE",
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_17c73037882f4fef8c9e4c51baddbe54",
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
        "value": "Liam Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Brown",
    "email": {
      "value": "alex.4419@testmail.io"
    },
    "id": "cust_60bd34433de643e7995281498c17fe67",
    "phone_number": "+441116537618",
    "connector_customer_id": "cus_UCZrJYMJG3QxmC"
  },
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
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "1231 Oak St"
      },
      "line2": {
        "value": "429 Oak Dr"
      },
      "line3": {
        "value": "7396 Oak Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33270"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4833@sandbox.example.com"
      },
      "phone_number": {
        "value": "6468420307"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9811 Market Blvd"
      },
      "line2": {
        "value": "1754 Oak St"
      },
      "line3": {
        "value": "6444 Main Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "17801"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.5076@example.com"
      },
      "phone_number": {
        "value": "2490427527"
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
  "description": "No3DS auto capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:01 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TEAhwD5R7gDAGff0xLrEfaf",
  "connectorTransactionId": "pi_3TEAhwD5R7gDAGff0xLrEfaf",
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
    "content-length": "5530",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:01 GMT",
    "idempotency-key": "03a735cd-f18e-40d5-913a-01645721a5fa",
    "original-request": "req_LHyB1zHWPD7Zlm",
    "request-id": "req_LHyB1zHWPD7Zlm",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "networkTransactionId": "976910110049114",
  "state": {
    "connectorCustomerId": "cus_UCZrJYMJG3QxmC"
  },
  "rawConnectorResponse": "***MASKED***"
  
  "capturedAmount": "6000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEAhwD5R7gDAGffFao8zDTb",
      "paymentMethodId": "pm_1TEAhwD5R7gDAGffFao8zDTb"
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
<summary>3. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_0c1e2fc80db14a758e7edc2146715ffa",
  "connector_transaction_id": "pi_3TEAhwD5R7gDAGff0xLrEfaf",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "cus_UCZrJYMJG3QxmC"
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:03 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "re_3TEAhwD5R7gDAGff0pVLP1Xu",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "714",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:03 GMT",
    "idempotency-key": "2110b235-fe69-4e9c-a816-562e7c6b2135",
    "original-request": "req_bucQFuh8bGdd4D",
    "request-id": "req_bucQFuh8bGdd4D",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "connectorTransactionId": "pi_3TEAhwD5R7gDAGff0xLrEfaf",
  "rawConnectorResponse": "***MASKED***"
  
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
  -H "x-request-id: refund_sync_refund_sync_with_reason_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "pi_3TEAhwD5R7gDAGff0xLrEfaf",
  "refund_id": "re_3TEAhwD5R7gDAGff0pVLP1Xu",
  "state": {
    "connector_customer_id": "cus_UCZrJYMJG3QxmC"
  },
  "refund_reason": "customer_requested"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:03 GMT
x-request-id: refund_sync_refund_sync_with_reason_req

Response contents:
{
  "merchantRefundId": "re_3TEAhwD5R7gDAGff0pVLP1Xu",
  "connectorRefundId": "re_3TEAhwD5R7gDAGff0pVLP1Xu",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "755",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:03 GMT",
    "request-id": "req_ShxajYCJaTWDah",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "connectorTransactionId": "pi_3TEAhwD5R7gDAGff0xLrEfaf",
  "rawConnectorResponse": "***MASKED***"
  
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)