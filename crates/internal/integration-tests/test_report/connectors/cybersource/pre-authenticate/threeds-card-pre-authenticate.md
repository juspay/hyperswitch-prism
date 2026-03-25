# Connector `cybersource` / Suite `pre_authenticate` / Scenario `threeds_card_pre_authenticate`

- Service: `PaymentMethodAuthenticationService/PreAuthenticate`
- PM / PMT: `card` / `credit`
- Result: `PASS`

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
  "merchant_order_id": "gen_625990",
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
        "value": "Mia Brown"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "casey.8554@sandbox.example.com"
    },
    "id": "cust_64097c75c30341ac929d6dda",
    "phone_number": "+18895458737"
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
        "value": "7966 Sunset Rd"
      },
      "line2": {
        "value": "2 Lake Blvd"
      },
      "line3": {
        "value": "3303 Sunset Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "49035"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5735@example.com"
      },
      "phone_number": {
        "value": "2408036188"
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
        "value": "1870 Market Dr"
      },
      "line2": {
        "value": "1529 Main Blvd"
      },
      "line3": {
        "value": "6190 Sunset Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "88751"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6256@testmail.io"
      },
      "phone_number": {
        "value": "1753517046"
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
    "v-c-correlation-id": "06c4dbbf-b59a-4155-9d95-54f234fc6dd1",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-22555856",
    "x-requestid": "7742912489396214704807",
    "x-response-time": "75ms"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://centinelapistag.cardinalcommerce.com/V1/Cruise/Collect",
      "method": "HTTP_METHOD_POST",
      "formFields": {
        "access_token": ***MASKED***"
        "ddc_url": "https://centinelapistag.cardinalcommerce.com/V1/Cruise/Collect",
        "reference_id": "64af5981-f54c-4b86-b553-09aee288e89d"
      }
    }
  },
  "merchantOrderId": "gen_625990",
  "rawConnectorResponse": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../pre-authenticate.md) | [Back to Overview](../../../test_overview.md)
