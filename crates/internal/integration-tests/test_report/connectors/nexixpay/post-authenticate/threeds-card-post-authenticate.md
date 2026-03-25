# Connector `nexixpay` / Suite `post_authenticate` / Scenario `threeds_card_post_authenticate`

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
x-connector-request-reference-id: gen_114514
x-merchant-id: test_merchant
x-request-id: post_authenticate_threeds_card_post_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:43 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: post_authenticate_threeds_card_post_authenticate_req" \
  -H "x-connector-request-reference-id: gen_114514" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/PostAuthenticate <<'JSON'
{
  "merchant_order_id": "gen_114514",
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
        "value": "Noah Brown"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Emma Brown",
    "email": {
      "value": "casey.3508@testmail.io"
    },
    "id": "cust_87ed3c3ea0434105aa48acdb",
    "phone_number": "+446491919049"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1329 Lake Ln"
      },
      "line2": {
        "value": "3804 Main Ln"
      },
      "line3": {
        "value": "9553 Oak Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "81271"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.7185@testmail.io"
      },
      "phone_number": {
        "value": "2882007634"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4603 Pine Ln"
      },
      "line2": {
        "value": "626 Market St"
      },
      "line3": {
        "value": "7488 Lake Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "22922"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3647@sandbox.example.com"
      },
      "phone_number": {
        "value": "6084921916"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_893ec8a0b02c48a18e6a6aa1"
  },
  "connector_order_reference_id": "gen_237615",
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
    "params": "gen_447028",
    "payload": {
      "transaction_id": "gen_901331",
      "PaRes": "gen_155012",
      "paymentId": "gen_110834"
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
x-connector-request-reference-id: gen_114514
x-merchant-id: test_merchant
x-request-id: post_authenticate_threeds_card_post_authenticate_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:59:43 GMT
x-request-id: post_authenticate_threeds_card_post_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../post-authenticate.md) | [Back to Overview](../../../test_overview.md)
