# Connector `stripe` / Suite `complete_authorize` / Scenario `Credit Card | 3DS`

- Service: `PaymentService/Authorize`
- Scenario Key: `threeds_complete_authorize_credit_card`
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
  "merchant_transaction_id": "mti_be5a8d6e0b854b7a88f24bc5ee53e89e",
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
        "value": "Noah Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.1451@testmail.io"
    },
    "id": "cust_8490cee5288d404382ce0c91d51b9101",
    "phone_number": "+446963377961"
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
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7286 Pine Dr"
      },
      "line2": {
        "value": "5679 Lake St"
      },
      "line3": {
        "value": "6411 Market Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "15075"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8103@example.com"
      },
      "phone_number": {
        "value": "4243483341"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2629 Pine Ln"
      },
      "line2": {
        "value": "4134 Lake Dr"
      },
      "line3": {
        "value": "5023 Market St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70854"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5957@sandbox.example.com"
      },
      "phone_number": {
        "value": "4531143641"
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
date: Mon, 23 Mar 2026 15:46:46 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TEAgkD5R7gDAGff19b0iCOY",
  "connectorTransactionId": "pi_3TEAgkD5R7gDAGff19b0iCOY",
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
    "content-length": "2302",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:46:46 GMT",
    "idempotency-key": "8f728cae-5295-42c3-aa99-8aef442eda7a",
    "original-request": "req_JUC4T4egZmnBh4",
    "request-id": "req_JUC4T4egZmnBh4",
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
        "payment_intent": "pi_3TEAgkD5R7gDAGff19b0iCOY",
        "payment_intent_client_secret": ***MASKED***"
        "publishable_key": "pk_test_51M7fTaD5R7gDAGffAofar8mp1iheEOKC3ZFXJcKdTCnWXBFgcxqOMt5zFCswSHP9zy1KzrjctJIQCYK1h7le3dAb00O0zVhQBY",
        "source": "payatt_3TEAgkD5R7gDAGff1yRpGCNL"
      }
    }
  },
  "rawConnectorResponse": "***MASKED***"
  
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEAgkD5R7gDAGffCN0JA2VZ",
      "paymentMethodId": "pm_1TEAgkD5R7gDAGffCN0JA2VZ"
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
  "merchant_order_id": "pi_3TEAgkD5R7gDAGff19b0iCOY",
  "merchant_transaction_id": "pi_3TEAgkD5R7gDAGff19b0iCOY",
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
        "value": "Noah Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "sam.1451@testmail.io"
    },
    "id": "cust_8490cee5288d404382ce0c91d51b9101",
    "phone_number": "+446963377961"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7286 Pine Dr"
      },
      "line2": {
        "value": "5679 Lake St"
      },
      "line3": {
        "value": "6411 Market Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "15075"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8103@example.com"
      },
      "phone_number": {
        "value": "4243483341"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2629 Pine Ln"
      },
      "line2": {
        "value": "4134 Lake Dr"
      },
      "line3": {
        "value": "5023 Market St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70854"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5957@sandbox.example.com"
      },
      "phone_number": {
        "value": "4531143641"
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
  "connector_transaction_id": "pi_3TEAgkD5R7gDAGff19b0iCOY",
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
date: Mon, 23 Mar 2026 15:47:10 GMT
x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TEAh8D5R7gDAGff0ksc01LI",
  "connectorTransactionId": "pi_3TEAh8D5R7gDAGff0ksc01LI",
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
    "content-length": "2293",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:47:10 GMT",
    "idempotency-key": "9b0a2526-96ed-4f3c-9641-da21bfa3c8b4",
    "original-request": "req_opq6Cp8CJ4Ejlw",
    "request-id": "req_opq6Cp8CJ4Ejlw",
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
        "payment_intent": "pi_3TEAh8D5R7gDAGff0ksc01LI",
        "payment_intent_client_secret": ***MASKED***"
        "publishable_key": "pk_test_51M7fTaD5R7gDAGffAofar8mp1iheEOKC3ZFXJcKdTCnWXBFgcxqOMt5zFCswSHP9zy1KzrjctJIQCYK1h7le3dAb00O0zVhQBY",
        "source": "payatt_3TEAh8D5R7gDAGff0y4O09J7"
      }
    }
  },
  "rawConnectorResponse": "***MASKED***"
  
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TEAh8D5R7gDAGffxnQbWmwZ",
      "paymentMethodId": "pm_1TEAh8D5R7gDAGffxnQbWmwZ"
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
date: Mon, 23 Mar 2026 15:47:11 GMT
x-request-id: get_sync_payment_req

Response contents:
{
  "connectorTransactionId": "pi_3TEAgkD5R7gDAGff19b0iCOY",
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
    "content-length": "5817",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=hUKDJ4YUx473XYLV0pYtoq3cs3AKKgls3BBntBphBU-yO5VTy8MFHoCN9NkxP-8p_wCp63JSrVwAdn07",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 15:47:11 GMT",
    "request-id": "req_bt9Mhk1sZrnenf",
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
      "connectorMandateId": "pm_1TEAgkD5R7gDAGffCN0JA2VZ",
      "paymentMethodId": "pm_1TEAgkD5R7gDAGffCN0JA2VZ"
    }
  },
  "networkTransactionId": "749866105705510",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "authenticationData": "eyJhdXRoZW50aWNhdGlvbl9mbG93IjoiY2hhbGxlbmdlIiwiZWxlY3Ryb25pY19jb21tZXJjZV9pbmRpY2F0b3IiOiIwNSIsImV4ZW1wdGlvbl9pbmRpY2F0b3IiOm51bGwsInJlc3VsdCI6ImF1dGhlbnRpY2F0ZWQiLCJyZXN1bHRfcmVhc29uIjpudWxsLCJ0cmFuc2FjdGlvbl9pZCI6ImVkZjg3YThlLTM0ZjktNDU1Yi05ZjY2LWQ1MWZjZjkwZThjYyIsInZlcnNpb24iOiIyLjIuMCJ9",
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
  
  "merchantTransactionId": "pi_3TEAgkD5R7gDAGff19b0iCOY"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../complete-authorize.md) | [Back to Overview](../../../test_overview.md)