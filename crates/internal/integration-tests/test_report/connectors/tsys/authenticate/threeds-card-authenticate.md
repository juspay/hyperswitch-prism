# Connector `tsys` / Suite `authenticate` / Scenario `threeds_card_authenticate`

- Service: `PaymentMethodAuthenticationService/Authenticate`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Execute 3DS challenge or frictionless verification. Authenticates customer
// via bank challenge or behind-the-scenes verification for fraud prevention.
rpc Authenticate ( .types.PaymentMethodAuthenticationServiceAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServiceAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authenticate_threeds_card_authenticate_ref
x-merchant-id: test_merchant
x-request-id: authenticate_threeds_card_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:38:29 GMT
x-request-id: authenticate_threeds_card_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

**Pre Requisites Executed**

<details>
<summary>1. pre_authenticate(threeds_card_pre_authenticate) — FAIL</summary>

**Dependency Error**

```text
Resolved method descriptor:
// Initiate 3DS flow before payment authorization. Collects device data and
// prepares authentication context for frictionless or challenge-based verification.
rpc PreAuthenticate ( .types.PaymentMethodAuthenticationServicePreAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServicePreAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: pre_authenticate_threeds_card_pre_authenticate_ref
x-merchant-id: test_merchant
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:38:29 GMT
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: pre_authenticate_threeds_card_pre_authenticate_req" \
  -H "x-connector-request-reference-id: pre_authenticate_threeds_card_pre_authenticate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/PreAuthenticate <<'JSON'
{
  "merchant_order_id": "gen_728353",
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
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "casey.3524@example.com"
    },
    "id": "cust_cbe93776b6914eb79b3983d1e6ca234d",
    "phone_number": "+18507346332"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8823 Main Rd"
      },
      "line2": {
        "value": "209 Market Ave"
      },
      "line3": {
        "value": "4895 Main Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63288"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6978@example.com"
      },
      "phone_number": {
        "value": "6957518400"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "609 Main Rd"
      },
      "line2": {
        "value": "6617 Oak St"
      },
      "line3": {
        "value": "9083 Lake Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "13378"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1861@sandbox.example.com"
      },
      "phone_number": {
        "value": "8144030674"
      },
      "phone_country_code": "+91"
    }
  },
  "enrolled_for_3ds": true,
  "metadata": {
    "value": "{}"
  },
  "connector_feature_data": {
    "value": "{}"
  },
  "return_url": "https://example.com/payment/return",
  "continue_redirection_url": "https://example.com/payment/complete",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US"
  },
  "description": "3DS pre-authenticate card payment",
  "capture_method": "MANUAL"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate 3DS flow before payment authorization. Collects device data and
// prepares authentication context for frictionless or challenge-based verification.
rpc PreAuthenticate ( .types.PaymentMethodAuthenticationServicePreAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServicePreAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: pre_authenticate_threeds_card_pre_authenticate_ref
x-merchant-id: test_merchant
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:38:29 GMT
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authenticate_threeds_card_authenticate_req" \
  -H "x-connector-request-reference-id: authenticate_threeds_card_authenticate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/Authenticate <<'JSON'
{
  "merchant_order_id": "gen_728353",
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
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "casey.3524@example.com"
    },
    "id": "cust_cbe93776b6914eb79b3983d1e6ca234d",
    "phone_number": "+18507346332"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8823 Main Rd"
      },
      "line2": {
        "value": "209 Market Ave"
      },
      "line3": {
        "value": "4895 Main Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63288"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6978@example.com"
      },
      "phone_number": {
        "value": "6957518400"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "609 Main Rd"
      },
      "line2": {
        "value": "6617 Oak St"
      },
      "line3": {
        "value": "9083 Lake Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "13378"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1861@sandbox.example.com"
      },
      "phone_number": {
        "value": "8144030674"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_a5bf33a69f6543458ecaed4a20b9bb91"
  },
  "metadata": {
    "value": "{}"
  },
  "connector_feature_data": {
    "value": "{}"
  },
  "return_url": "https://example.com/payment/return",
  "continue_redirection_url": "https://example.com/payment/complete",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US"
  },
  "capture_method": "MANUAL",
  "redirection_response": {
    "params": "gen_568324",
    "payload": {
      "transaction_id": "gen_474690"
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
// Execute 3DS challenge or frictionless verification. Authenticates customer
// via bank challenge or behind-the-scenes verification for fraud prevention.
rpc Authenticate ( .types.PaymentMethodAuthenticationServiceAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServiceAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authenticate_threeds_card_authenticate_ref
x-merchant-id: test_merchant
x-request-id: authenticate_threeds_card_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:38:29 GMT
x-request-id: authenticate_threeds_card_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../authenticate.md) | [Back to Overview](../../../test_overview.md)
