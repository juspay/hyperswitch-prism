# Connector `shift4` / Suite `authenticate` / Scenario `threeds_card_authenticate`

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
date: Mon, 23 Mar 2026 16:30:52 GMT
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
date: Mon, 23 Mar 2026 16:30:52 GMT
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
  "merchant_order_id": "gen_763420",
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
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "morgan.7896@sandbox.example.com"
    },
    "id": "cust_148d70d9a63a4fcab4f688d85ee63c14",
    "phone_number": "+14246118715"
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
        "value": "1413 Pine Dr"
      },
      "line2": {
        "value": "4091 Market Ln"
      },
      "line3": {
        "value": "7479 Main St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "58581"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4645@sandbox.example.com"
      },
      "phone_number": {
        "value": "8081454627"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2434 Sunset Ave"
      },
      "line2": {
        "value": "2472 Market Ln"
      },
      "line3": {
        "value": "1622 Oak Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96148"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3668@sandbox.example.com"
      },
      "phone_number": {
        "value": "5047223395"
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
date: Mon, 23 Mar 2026 16:30:52 GMT
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
  "merchant_order_id": "gen_763420",
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
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "morgan.7896@sandbox.example.com"
    },
    "id": "cust_148d70d9a63a4fcab4f688d85ee63c14",
    "phone_number": "+14246118715"
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
        "value": "1413 Pine Dr"
      },
      "line2": {
        "value": "4091 Market Ln"
      },
      "line3": {
        "value": "7479 Main St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "58581"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4645@sandbox.example.com"
      },
      "phone_number": {
        "value": "8081454627"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2434 Sunset Ave"
      },
      "line2": {
        "value": "2472 Market Ln"
      },
      "line3": {
        "value": "1622 Oak Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96148"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3668@sandbox.example.com"
      },
      "phone_number": {
        "value": "5047223395"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_f896e013f3554166b88271b90c5ce5ff"
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
    "params": "gen_331464",
    "payload": {
      "transaction_id": "gen_489673"
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
date: Mon, 23 Mar 2026 16:30:52 GMT
x-request-id: authenticate_threeds_card_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../authenticate.md) | [Back to Overview](../../../test_overview.md)
