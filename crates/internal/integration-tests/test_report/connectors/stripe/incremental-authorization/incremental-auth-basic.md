# Connector `stripe` / Suite `incremental_authorization` / Scenario `Incremental Authorization | Incremental Auth Basic`

- Service: `Unknown`
- Scenario Key: `incremental_auth_basic`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_authorization_id': ***MASKED***
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_7cb4dd762e1945f58437ac2e97df7cef",
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
        "value": "Ethan Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.5778@testmail.io"
    },
    "id": "cust_f7c23e0e50d1494788a6e8c309089290",
    "phone_number": "+15578423039"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "7736 Main Ave"
      },
      "line2": {
        "value": "8618 Sunset Dr"
      },
      "line3": {
        "value": "1468 Sunset St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "37008"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9496@testmail.io"
      },
      "phone_number": {
        "value": "2770757091"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "7117 Market St"
      },
      "line2": {
        "value": "6897 Sunset Ln"
      },
      "line3": {
        "value": "5274 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32296"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8875@example.com"
      },
      "phone_number": {
        "value": "8545035933"
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:24 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TEAiJD5R7gDAGff1k6NfPEB",
  "connectorTransactionId": "pi_3TEAiJD5R7gDAGff1k6NfPEB",
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
    "content-length": "5523",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:24 GMT",
    "idempotency-key": "3d8e2e3f-521b-4534-9333-0017e88ba452",
    "original-request": "req_vIQ0LlW1k5uLZh",
    "request-id": "req_vIQ0LlW1k5uLZh",
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
  "rawConnectorResponse": "***MASKED***"
  
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEAiJD5R7gDAGffMqTXTzGZ",
      "paymentMethodId": "pm_1TEAiJD5R7gDAGffMqTXTzGZ"
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
  -H "x-request-id: incremental_authorization_incremental_auth_basic_req" \
  -H "x-connector-request-reference-id: incremental_authorization_incremental_auth_basic_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/IncrementalAuthorization <<'JSON'
{
  "merchant_authorization_id": ***MASKED***"
  "connector_transaction_id": "pi_3TEAiJD5R7gDAGff1k6NfPEB",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "reason": "Additional authorization for tip"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Increase authorized amount if still in authorized state. Allows adding
// charges to existing authorization for hospitality, tips, or incremental services.
rpc IncrementalAuthorization ( .types.PaymentServiceIncrementalAuthorizationRequest ) returns ( .types.PaymentServiceIncrementalAuthorizationResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: incremental_authorization_incremental_auth_basic_ref
x-merchant-id: test_merchant
x-request-id: incremental_authorization_incremental_auth_basic_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:25 GMT
x-request-id: incremental_authorization_incremental_auth_basic_req

Response contents:
{
  "status": "AUTHORIZATION_FAILURE",
  "error": {
    "connectorDetails": {
      "code": "No error code",
      "message": "This PaymentIntent is not eligible for incremental authorization because you did not request support using `request_incremental_authorization` when you created or confirmed it.",
      "reason": "This PaymentIntent is not eligible for incremental authorization because you did not request support using `request_incremental_authorization` when you created or confirmed it."
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
    "content-length": "2510",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:25 GMT",
    "idempotency-key": "2950178e-4816-4e8d-93a2-d2ab7a0b50fb",
    "original-request": "req_yIZmTtOUTAeg3r",
    "request-id": "req_yIZmTtOUTAeg3r",
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


[Back to Connector Suite](../incremental-authorization.md) | [Back to Overview](../../../test_overview.md)