# Connector `stripe` / Suite `revoke_mandate` / Scenario `Revoke Mandate | Revoke Active Mandate`

- Service: `Unknown`
- Scenario Key: `revoke_active_mandate`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Cancel an existing recurring payment mandate. Stops future automatic
// charges on customer's stored consent for subscription cancellations.
rpc Revoke ( .types.RecurringPaymentServiceRevokeRequest ) returns ( .types.RecurringPaymentServiceRevokeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: revoke_mandate_revoke_active_mandate_ref
x-merchant-id: test_merchant
x-request-id: revoke_mandate_revoke_active_mandate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:22 GMT
x-request-id: revoke_mandate_revoke_active_mandate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

**Pre Requisites Executed**

<details>
<summary>1. setup_recurring(setup_recurring) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_3ea1256f30d64b91b92f32997b3c2364",
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
    "name": "Mia Wilson",
    "email": {
      "value": "casey.8260@testmail.io"
    },
    "id": "cust_a2c1f5a0d2294d3eb702ce46ca315435",
    "phone_number": "+919989610046"
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
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6699 Main Ln"
      },
      "line2": {
        "value": "5390 Sunset St"
      },
      "line3": {
        "value": "447 Pine Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99620"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6136@testmail.io"
      },
      "phone_number": {
        "value": "4817019802"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:22 GMT
x-request-id: setup_recurring_setup_recurring_req

Response contents:
{
  "connectorRecurringPaymentId": "seti_1TEAiHD5R7gDAGffvvGxYiLM",
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
    "content-length": "1965",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:48:22 GMT",
    "idempotency-key": "9915d95e-d308-4074-8d08-05b4d37201d6",
    "original-request": "req_nB8oKt0i9dbqjF",
    "request-id": "req_nB8oKt0i9dbqjF",
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
      "connectorMandateId": "pm_1TEAiHD5R7gDAGffJ8cmel4E",
      "paymentMethodId": "pm_1TEAiHD5R7gDAGffJ8cmel4E"
    }
  },
  "merchantRecurringPaymentId": "seti_1TEAiHD5R7gDAGffvvGxYiLM",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjpudWxsLCJhZGRyZXNzX3Bvc3RhbF9jb2RlX2NoZWNrIjpudWxsLCJjdmNfY2hlY2siOiJwYXNzIn0="
      }
    }
  },
  "capturedAmount": "6000",
  "rawConnectorRequest": "***MASKED***"


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
  -H "x-request-id: revoke_mandate_revoke_active_mandate_req" \
  -H "x-connector-request-reference-id: revoke_mandate_revoke_active_mandate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RecurringPaymentService/Revoke <<'JSON'
{
  "merchant_revoke_id": "gen_411520",
  "mandate_id": "gen_216761",
  "connector_mandate_id": "gen_841565"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Cancel an existing recurring payment mandate. Stops future automatic
// charges on customer's stored consent for subscription cancellations.
rpc Revoke ( .types.RecurringPaymentServiceRevokeRequest ) returns ( .types.RecurringPaymentServiceRevokeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: revoke_mandate_revoke_active_mandate_ref
x-merchant-id: test_merchant
x-request-id: revoke_mandate_revoke_active_mandate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 15:48:22 GMT
x-request-id: revoke_mandate_revoke_active_mandate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../revoke-mandate.md) | [Back to Overview](../../../test_overview.md)