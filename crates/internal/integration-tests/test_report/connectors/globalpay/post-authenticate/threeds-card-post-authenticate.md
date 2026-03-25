# Connector `globalpay` / Suite `post_authenticate` / Scenario `threeds_card_post_authenticate`

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
date: Mon, 23 Mar 2026 18:51:58 GMT
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
date: Mon, 23 Mar 2026 18:51:58 GMT
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
  "merchant_order_id": "gen_195286",
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
        "value": "Liam Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "sam.5706@sandbox.example.com"
    },
    "id": "cust_06793f88cf1941e99e05c953",
    "phone_number": "+19941170112"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6464 Market Ln"
      },
      "line2": {
        "value": "8290 Pine Blvd"
      },
      "line3": {
        "value": "6754 Lake St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74331"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9015@example.com"
      },
      "phone_number": {
        "value": "6280165892"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "7750 Market St"
      },
      "line2": {
        "value": "5279 Main Ln"
      },
      "line3": {
        "value": "2676 Lake Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60189"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5731@example.com"
      },
      "phone_number": {
        "value": "7950235659"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_35a27bf2c0964d3184e34a62"
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
    "params": "gen_998633",
    "payload": {
      "transaction_id": "gen_970558"
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
date: Mon, 23 Mar 2026 18:51:58 GMT
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
  "merchant_order_id": "gen_195286",
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
        "value": "Liam Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "sam.5706@sandbox.example.com"
    },
    "id": "cust_06793f88cf1941e99e05c953",
    "phone_number": "+19941170112"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6464 Market Ln"
      },
      "line2": {
        "value": "8290 Pine Blvd"
      },
      "line3": {
        "value": "6754 Lake St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74331"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9015@example.com"
      },
      "phone_number": {
        "value": "6280165892"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "7750 Market St"
      },
      "line2": {
        "value": "5279 Main Ln"
      },
      "line3": {
        "value": "2676 Lake Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60189"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5731@example.com"
      },
      "phone_number": {
        "value": "7950235659"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_35a27bf2c0964d3184e34a62"
  },
  "connector_order_reference_id": "gen_502243",
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
    "params": "gen_998633",
    "payload": {
      "transaction_id": "gen_970558"
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
date: Mon, 23 Mar 2026 18:51:58 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../post-authenticate.md) | [Back to Overview](../../../test_overview.md)
