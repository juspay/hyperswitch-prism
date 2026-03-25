# Connector `stripe` / Suite `complete_authorize` / Scenario `threeds_complete_authorize_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. authorize(threeds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c8a49d10f40541bda0a1049b",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000002760003184"
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
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.2230@example.com"
    },
    "id": "cust_8a8bf076d5124a75ac27f748",
    "phone_number": "+14436463175"
  },
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
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "730 Lake Rd"
      },
      "line2": {
        "value": "6247 Sunset Rd"
      },
      "line3": {
        "value": "1364 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59401"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6769@testmail.io"
      },
      "phone_number": {
        "value": "9852738420"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1177 Oak Blvd"
      },
      "line2": {
        "value": "647 Lake Rd"
      },
      "line3": {
        "value": "9160 Oak St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67900"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1134@example.com"
      },
      "phone_number": {
        "value": "8599640946"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
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
x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:49:02 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
  "connectorTransactionId": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
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
    "content-length": "2296",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:49:02 GMT",
    "idempotency-key": "05678375-204a-4cea-a579-8c90f15c83fa",
    "original-request": "req_sNbZ13Qj8Cp3tc",
    "request-id": "req_sNbZ13Qj8Cp3tc",
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
      "endpoint": "https://hooks.stripe.com/3d_secure_2/hosted",
      "method": "HTTP_METHOD_GET",
      "formFields": {
        "merchant": "acct_1M7fTaD5R7gDAGff",
        "payment_intent": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
        "payment_intent_client_secret": ***MASKED***"
        "publishable_key": "pk_test_51M7fTaD5R7gDAGffAofar8mp1iheEOKC3ZFXJcKdTCnWXBFgcxqOMt5zFCswSHP9zy1KzrjctJIQCYK1h7le3dAb00O0zVhQBY",
        "source": "payatt_3TEQe1D5R7gDAGff0AQcGh5F"
      }
    }
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEQe1D5R7gDAGff0MtI8vts",
      "paymentMethodId": "pm_1TEQe1D5R7gDAGff0MtI8vts"
    }
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
  -H "x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req" \
  -H "x-connector-request-reference-id: complete_authorize_threeds_complete_authorize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_order_id": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
  "merchant_transaction_id": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000002760003184"
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
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.2230@example.com"
    },
    "id": "cust_8a8bf076d5124a75ac27f748",
    "phone_number": "+14436463175"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "730 Lake Rd"
      },
      "line2": {
        "value": "6247 Sunset Rd"
      },
      "line3": {
        "value": "1364 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59401"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6769@testmail.io"
      },
      "phone_number": {
        "value": "9852738420"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1177 Oak Blvd"
      },
      "line2": {
        "value": "647 Lake Rd"
      },
      "line3": {
        "value": "9160 Oak St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67900"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1134@example.com"
      },
      "phone_number": {
        "value": "8599640946"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
}
JSON

# follow-up sync request
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: get_sync_payment_req" \
  -H "x-connector-request-reference-id: get_sync_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "",
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": ""
      },
      "token_type": ***MASKED***"
      "expires_in_seconds": 0
    }
  }
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
x-connector-request-reference-id: complete_authorize_threeds_complete_authorize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:49:14 GMT
x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TEQeDD5R7gDAGff09ZrUGSr",
  "connectorTransactionId": "pi_3TEQeDD5R7gDAGff09ZrUGSr",
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
    "content-length": "2295",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:49:14 GMT",
    "idempotency-key": "2176f9ca-044d-4e66-bb9b-6b83c6997c8a",
    "original-request": "req_yr3WO1g6DAeWrg",
    "request-id": "req_yr3WO1g6DAeWrg",
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
      "endpoint": "https://hooks.stripe.com/3d_secure_2/hosted",
      "method": "HTTP_METHOD_GET",
      "formFields": {
        "merchant": "acct_1M7fTaD5R7gDAGff",
        "payment_intent": "pi_3TEQeDD5R7gDAGff09ZrUGSr",
        "payment_intent_client_secret": ***MASKED***"
        "publishable_key": "pk_test_51M7fTaD5R7gDAGffAofar8mp1iheEOKC3ZFXJcKdTCnWXBFgcxqOMt5zFCswSHP9zy1KzrjctJIQCYK1h7le3dAb00O0zVhQBY",
        "source": "payatt_3TEQeDD5R7gDAGff0t3nQ64R"
      }
    }
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEQeDD5R7gDAGffzBJG1meh",
      "paymentMethodId": "pm_1TEQeDD5R7gDAGffzBJG1meh"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response

# follow-up sync response
Resolved method descriptor:
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:49:14 GMT
x-request-id: get_sync_payment_req

Response contents:
{
  "connectorTransactionId": "pi_3TEQe1D5R7gDAGff0DQVzXZ8",
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
    "content-length": "5804",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=cf2m0QZllQ_cO_-ua0bmze_ZEC6zU5cKsor4N_WReyb_KG6mR6TnzmyVa9YaYVkePdSCzFeftRkot2ZX",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 08:49:14 GMT",
    "request-id": "req_uxkezXCctaLhS9",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEQe1D5R7gDAGff0MtI8vts",
      "paymentMethodId": "pm_1TEQe1D5R7gDAGff0MtI8vts"
    }
  },
  "networkTransactionId": "749866105705510",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "authenticationData": "eyJhdXRoZW50aWNhdGlvbl9mbG93IjoiY2hhbGxlbmdlIiwiZWxlY3Ryb25pY19jb21tZXJjZV9pbmRpY2F0b3IiOiIwNSIsImV4ZW1wdGlvbl9pbmRpY2F0b3IiOm51bGwsInJlc3VsdCI6ImF1dGhlbnRpY2F0ZWQiLCJyZXN1bHRfcmVhc29uIjpudWxsLCJ0cmFuc2FjdGlvbl9pZCI6ImUwNDgzMTI1LWYzYzEtNDVmNC1iMjI1LTczOTc1NGMyMzdiZCIsInZlcnNpb24iOiIyLjIuMCJ9",
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjoicGFzcyIsImFkZHJlc3NfcG9zdGFsX2NvZGVfY2hlY2siOiJwYXNzIiwiY3ZjX2NoZWNrIjoicGFzcyJ9"
      }
    },
    "extendedAuthorizationResponseData": ***MASKED***
      "extendedAuthenticationApplied": false
    },
    "isOvercaptureEnabled": false
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
      "expiresInSeconds": "0",
      "tokenType": ***MASKED***"
    }
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  },
  "merchantTransactionId": "pi_3TEQe1D5R7gDAGff0DQVzXZ8"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../complete-authorize.md) | [Back to Overview](../../../test_overview.md)
