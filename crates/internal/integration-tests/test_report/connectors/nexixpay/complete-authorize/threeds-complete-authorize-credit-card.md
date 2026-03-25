# Connector `nexixpay` / Suite `complete_authorize` / Scenario `threeds_complete_authorize_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
x-connector-request-reference-id: gen_182911
x-merchant-id: test_merchant
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:44 GMT
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
  -H "x-connector-request-reference-id: gen_182911" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/PreAuthenticate <<'JSON'
{
  "merchant_order_id": "gen_182911",
  "amount": {
    "minor_amount": 100,
    "currency": "EUR"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4349940199004549"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "alex.5414@testmail.io"
    },
    "id": "cust_da68b39715934ea2accaf7e6",
    "phone_number": "+18631982592"
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
        "value": "8916 Main Blvd"
      },
      "line2": {
        "value": "9972 Lake Rd"
      },
      "line3": {
        "value": "3899 Lake Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59197"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1409@testmail.io"
      },
      "phone_number": {
        "value": "7315795493"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4818 Main Blvd"
      },
      "line2": {
        "value": "4911 Pine Ave"
      },
      "line3": {
        "value": "8162 Sunset St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99486"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8993@sandbox.example.com"
      },
      "phone_number": {
        "value": "4378524466"
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
x-connector-request-reference-id: gen_182911
x-merchant-id: test_merchant
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:44 GMT
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>

</details>
<details>
<summary>2. post_authenticate(threeds_card_post_authenticate) — FAIL</summary>

**Dependency Error**

```text
Resolved method descriptor:
// Validate authentication results with the issuing bank. Processes bank's
// authentication decision to determine if payment can proceed.
rpc PostAuthenticate ( .types.PaymentMethodAuthenticationServicePostAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServicePostAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: gen_182911
x-merchant-id: test_merchant
x-request-id: post_authenticate_threeds_card_post_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:44 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
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
  -H "x-request-id: post_authenticate_threeds_card_post_authenticate_req" \
  -H "x-connector-request-reference-id: gen_182911" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/PostAuthenticate <<'JSON'
{
  "merchant_order_id": "gen_182911",
  "amount": {
    "minor_amount": 100,
    "currency": "EUR"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4349940199004549"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "alex.5414@testmail.io"
    },
    "id": "cust_da68b39715934ea2accaf7e6",
    "phone_number": "+18631982592"
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
        "value": "8916 Main Blvd"
      },
      "line2": {
        "value": "9972 Lake Rd"
      },
      "line3": {
        "value": "3899 Lake Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59197"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1409@testmail.io"
      },
      "phone_number": {
        "value": "7315795493"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4818 Main Blvd"
      },
      "line2": {
        "value": "4911 Pine Ave"
      },
      "line3": {
        "value": "8162 Sunset St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99486"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8993@sandbox.example.com"
      },
      "phone_number": {
        "value": "4378524466"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_6cbeb0ef5df34be9917feb02"
  },
  "connector_order_reference_id": "gen_248771",
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
    "params": "gen_180373",
    "payload": {
      "transaction_id": "gen_593971",
      "PaRes": "gen_476551",
      "paymentId": "gen_746142"
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
// Validate authentication results with the issuing bank. Processes bank's
// authentication decision to determine if payment can proceed.
rpc PostAuthenticate ( .types.PaymentMethodAuthenticationServicePostAuthenticateRequest ) returns ( .types.PaymentMethodAuthenticationServicePostAuthenticateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: gen_182911
x-merchant-id: test_merchant
x-request-id: post_authenticate_threeds_card_post_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:44 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
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
  -H "x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req" \
  -H "x-connector-request-reference-id: gen_182911" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_order_id": "gen_182911",
  "merchant_transaction_id": "pay_1234567890",
  "amount": {
    "minor_amount": 100,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4349940199004549"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "alex.5414@testmail.io"
    },
    "id": "cust_da68b39715934ea2accaf7e6",
    "phone_number": "+18631982592"
  },
  "redirection_response": {
    "params": "gen_180373",
    "payload": {
      "transaction_id": "gen_593971",
      "PaRes": "gen_476551",
      "paymentId": "gen_746142"
    }
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
        "value": "8916 Main Blvd"
      },
      "line2": {
        "value": "9972 Lake Rd"
      },
      "line3": {
        "value": "3899 Lake Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59197"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1409@testmail.io"
      },
      "phone_number": {
        "value": "7315795493"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4818 Main Blvd"
      },
      "line2": {
        "value": "4911 Pine Ave"
      },
      "line3": {
        "value": "8162 Sunset St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99486"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8993@sandbox.example.com"
      },
      "phone_number": {
        "value": "4378524466"
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
  "description": "3DS pre-authenticate card payment",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US",
  "authentication_data": {
    "connector_transaction_id": "gen_746142"
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
x-connector-request-reference-id: gen_182911
x-merchant-id: test_merchant
x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:44 GMT
x-request-id: complete_authorize_threeds_complete_authorize_credit_card_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "INTERNAL_SERVER_ERROR",
      "message": "Failed to execute a processing step: None"
    }
  },
  "statusCode": 500
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../complete-authorize.md) | [Back to Overview](../../../test_overview.md)
