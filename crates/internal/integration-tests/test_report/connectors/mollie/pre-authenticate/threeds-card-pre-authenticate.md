# Connector `mollie` / Suite `pre_authenticate` / Scenario `threeds_card_pre_authenticate`

- Service: `PaymentMethodAuthenticationService/PreAuthenticate`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

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
date: Tue, 24 Mar 2026 07:03:51 GMT
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
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
  -H "x-request-id: pre_authenticate_threeds_card_pre_authenticate_req" \
  -H "x-connector-request-reference-id: pre_authenticate_threeds_card_pre_authenticate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/PreAuthenticate <<'JSON'
{
  "merchant_order_id": "gen_269794",
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
    "name": "Ava Wilson",
    "email": {
      "value": "casey.3782@sandbox.example.com"
    },
    "id": "cust_fc6465a0c74d4a1cb83ca488",
    "phone_number": "+16591871444"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9492 Pine Dr"
      },
      "line2": {
        "value": "1985 Sunset Rd"
      },
      "line3": {
        "value": "1780 Oak St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "79586"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4256@example.com"
      },
      "phone_number": {
        "value": "8341292057"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1606 Oak Ln"
      },
      "line2": {
        "value": "3322 Market Blvd"
      },
      "line3": {
        "value": "3738 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "17282"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3062@testmail.io"
      },
      "phone_number": {
        "value": "1732905959"
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
<summary>Show Response (masked)</summary>

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
date: Tue, 24 Mar 2026 07:03:51 GMT
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to execute a processing step: None
```

</details>


[Back to Connector Suite](../pre-authenticate.md) | [Back to Overview](../../../test_overview.md)
