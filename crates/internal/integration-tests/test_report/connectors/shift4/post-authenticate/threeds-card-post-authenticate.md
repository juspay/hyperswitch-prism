# Connector `shift4` / Suite `post_authenticate` / Scenario `threeds_card_post_authenticate`

- Service: `PaymentMethodAuthenticationService/PostAuthenticate`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Validate authentication results with the issuing bank. Processes bank's
// authentication decision to determine if payment can proceed.
rpc PostAuthenticate ( .types.PaymentMethodAuthenticationServicePostAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServicePostAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: post_authenticate_threeds_card_post_authenticate_ref
x-merchant-id: test_merchant
x-request-id: post_authenticate_threeds_card_post_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:31:03 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

**Pre Requisites Executed**

<details>
<summary>1. authenticate(threeds_card_authenticate) — FAIL</summary>

**Dependency Error**

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
date: Mon, 23 Mar 2026 16:31:02 GMT
x-request-id: authenticate_threeds_card_authenticate_req
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
  -H "x-request-id: authenticate_threeds_card_authenticate_req" \
  -H "x-connector-request-reference-id: authenticate_threeds_card_authenticate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/Authenticate <<'JSON'
{
  "merchant_order_id": "gen_608969",
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
        "value": "Emma Taylor"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ava Taylor",
    "email": {
      "value": "riley.2773@sandbox.example.com"
    },
    "id": "cust_619c77aee3404086b4af35cf41acd8f9",
    "phone_number": "+18139620141"
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
        "value": "9870 Oak Ln"
      },
      "line2": {
        "value": "6103 Lake Dr"
      },
      "line3": {
        "value": "5355 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30211"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3414@sandbox.example.com"
      },
      "phone_number": {
        "value": "9465255132"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2034 Sunset St"
      },
      "line2": {
        "value": "2165 Lake Ave"
      },
      "line3": {
        "value": "1928 Market Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21339"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5953@example.com"
      },
      "phone_number": {
        "value": "1753520450"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_ed6bb4d56c234506a227dcec0d759fdf"
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
    "params": "gen_759757",
    "payload": {
      "transaction_id": "gen_642244"
    }
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

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
date: Mon, 23 Mar 2026 16:31:02 GMT
x-request-id: authenticate_threeds_card_authenticate_req
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
  -H "x-request-id: post_authenticate_threeds_card_post_authenticate_req" \
  -H "x-connector-request-reference-id: post_authenticate_threeds_card_post_authenticate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/PostAuthenticate <<'JSON'
{
  "merchant_order_id": "gen_608969",
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
        "value": "Emma Taylor"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ava Taylor",
    "email": {
      "value": "riley.2773@sandbox.example.com"
    },
    "id": "cust_619c77aee3404086b4af35cf41acd8f9",
    "phone_number": "+18139620141"
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
        "value": "9870 Oak Ln"
      },
      "line2": {
        "value": "6103 Lake Dr"
      },
      "line3": {
        "value": "5355 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30211"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3414@sandbox.example.com"
      },
      "phone_number": {
        "value": "9465255132"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2034 Sunset St"
      },
      "line2": {
        "value": "2165 Lake Ave"
      },
      "line3": {
        "value": "1928 Market Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21339"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5953@example.com"
      },
      "phone_number": {
        "value": "1753520450"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_ed6bb4d56c234506a227dcec0d759fdf"
  },
  "connector_order_reference_id": "gen_845940",
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
  "redirection_response": {
    "params": "gen_759757",
    "payload": {
      "transaction_id": "gen_642244"
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
// Validate authentication results with the issuing bank. Processes bank's
// authentication decision to determine if payment can proceed.
rpc PostAuthenticate ( .types.PaymentMethodAuthenticationServicePostAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServicePostAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: post_authenticate_threeds_card_post_authenticate_ref
x-merchant-id: test_merchant
x-request-id: post_authenticate_threeds_card_post_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:31:03 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../post-authenticate.md) | [Back to Overview](../../../test_overview.md)
