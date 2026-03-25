# Connector `cybersource` / Suite `authenticate` / Scenario `threeds_card_authenticate`

- Service: `PaymentMethodAuthenticationService/Authenticate`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. pre_authenticate(threeds_card_pre_authenticate) — PASS</summary>

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
  "merchant_order_id": "gen_646349",
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
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "jordan.1325@example.com"
    },
    "id": "cust_a2d956da37024dbe94f202db",
    "phone_number": "+15075672456"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3074 Sunset Blvd"
      },
      "line2": {
        "value": "1912 Sunset Blvd"
      },
      "line3": {
        "value": "3321 Sunset Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74459"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2597@testmail.io"
      },
      "phone_number": {
        "value": "6225638201"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8087 Sunset Blvd"
      },
      "line2": {
        "value": "1014 Lake Ln"
      },
      "line3": {
        "value": "9277 Oak Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70028"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5165@example.com"
      },
      "phone_number": {
        "value": "1588571608"
      },
      "phone_country_code": "+91"
    }
  },
  "enrolled_for_3ds": true,
  "metadata": {
    "value": "{}"
  },
  "connector_feature_data": {
    "value": "{\"disable_avs\":false,\"disable_cvn\":false}"
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
content-type: application/grpc
date: Mon, 23 Mar 2026 18:40:49 GMT
x-request-id: pre_authenticate_threeds_card_pre_authenticate_req

Response contents:
{
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "763",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "00dfdccb-bd17-4b68-bf2a-9746eb449388",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-23753588",
    "x-requestid": "7742912494596530204805",
    "x-response-time": "80ms"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://centinelapistag.cardinalcommerce.com/V1/Cruise/Collect",
      "method": "HTTP_METHOD_POST",
      "formFields": {
        "access_token": ***MASKED***"
        "ddc_url": "https://centinelapistag.cardinalcommerce.com/V1/Cruise/Collect",
        "reference_id": "6724ccfb-a7cf-4878-9690-5165e02662c6"
      }
    }
  },
  "merchantOrderId": "gen_646349",
  "rawConnectorResponse": "***MASKED***"
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
  -H "x-request-id: authenticate_threeds_card_authenticate_req" \
  -H "x-connector-request-reference-id: authenticate_threeds_card_authenticate_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodAuthenticationService/Authenticate <<'JSON'
{
  "merchant_order_id": "gen_646349",
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
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "jordan.1325@example.com"
    },
    "id": "cust_a2d956da37024dbe94f202db",
    "phone_number": "+15075672456"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3074 Sunset Blvd"
      },
      "line2": {
        "value": "1912 Sunset Blvd"
      },
      "line3": {
        "value": "3321 Sunset Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74459"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2597@testmail.io"
      },
      "phone_number": {
        "value": "6225638201"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "8087 Sunset Blvd"
      },
      "line2": {
        "value": "1014 Lake Ln"
      },
      "line3": {
        "value": "9277 Oak Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70028"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5165@example.com"
      },
      "phone_number": {
        "value": "1588571608"
      },
      "phone_country_code": "+91"
    }
  },
  "authentication_data": {
    "connector_transaction_id": "cti_e7987488f3e0414fa2f3df10"
  },
  "metadata": {
    "value": "{}"
  },
  "connector_feature_data": {
    "value": "{\"disable_avs\":false,\"disable_cvn\":false}"
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
    "params": "6724ccfb-a7cf-4878-9690-5165e02662c6",
    "payload": {
      "transaction_id": "gen_955375"
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
content-type: application/grpc
date: Mon, 23 Mar 2026 18:40:50 GMT
x-request-id: authenticate_threeds_card_authenticate_req

Response contents:
{
  "status": "AUTHENTICATION_SUCCESSFUL",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "723",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "57b23e3d-f19b-4da3-a5d0-bfad836abd30",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-23753652",
    "x-requestid": "7742912499956530504805",
    "x-response-time": "98ms"
  },
  "merchantOrderId": "gen_646349",
  "authenticationData": {
    "eci": "internet",
    "messageVersion": "2.2.0"
  },
  "rawConnectorResponse": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authenticate.md) | [Back to Overview](../../../test_overview.md)
