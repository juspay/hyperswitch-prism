# Connector `paypal` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

- Service: `PaymentService/SetupRecurring`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. create_access_token(create_access_token) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: paypal" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_access_token_create_access_token_req" \
  -H "x-connector-request-reference-id: create_access_token_create_access_token_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateAccessToken <<'JSON'
{
  "merchant_access_token_id": ***MASKED***"
  "connector": "STRIPE",
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Generate short-lived connector authentication token. Provides secure
// credentials for connector API access without storing secrets client-side.
rpc CreateAccessToken ( .types.MerchantAuthenticationServiceCreateAccessTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateAccessTokenResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: create_access_token_create_access_token_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: create_access_token_create_access_token_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:17 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29549",
  "status": "OPERATION_STATUS_SUCCESS",
  "statusCode": 200
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
  -H "x-connector: paypal" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_with_webhook_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_be1b9375b4d74fe2981911fcf8b3ac04",
  "amount": {
    "minor_amount": 4500,
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
    "name": "Liam Wilson",
    "email": {
      "value": "alex.9826@sandbox.example.com"
    },
    "id": "cust_cee6134a7ab84e78b7562e624569cf61",
    "phone_number": "+13712337108"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29549"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "8424 Sunset Dr"
      },
      "line2": {
        "value": "7708 Pine St"
      },
      "line3": {
        "value": "3964 Pine Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71811"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9683@sandbox.example.com"
      },
      "phone_number": {
        "value": "8776529918"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION",
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_with_webhook_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:20 GMT
x-request-id: setup_recurring_setup_recurring_with_webhook_req

Response contents:
{
  "connectorRecurringPaymentId": "23f25213uv881292j",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "585",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:20 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f932344f32ccd",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f932344f32ccd-8a761f3c6497b2fe-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880075-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384500.140073,VS0,VE648"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "23f25213uv881292j"
    }
  },
  "merchantRecurringPaymentId": "23f25213uv881292j",
  "capturedAmount": "0",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29549"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v3/vault/payment-tokens/\",\"method\":\"POST\",\"headers\":{\"Prefer\":\"return=representation\",\"Content-Type\":\"application/json\",\"PayPal-Request-Id\":\"mrpi_be1b9375b4d74fe2981911fcf8b3ac04\",\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\"},\"body\":{\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"8424 Sunset Dr\",\"postal_code\":\"71811\",\"country_code\":\"US\",\"admin_area_2\":\"San Francisco\"},\"expiry\":\"2030-08\",\"name\":\"Emma Miller\",\"number\":\"4111111111111111\"}}}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
