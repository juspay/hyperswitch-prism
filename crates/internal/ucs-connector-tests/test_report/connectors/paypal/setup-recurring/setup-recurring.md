# Connector `paypal` / Suite `setup_recurring` / Scenario `setup_recurring`

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
  -H "x-request-id: setup_recurring_setup_recurring_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_ae4ef17307ba43f39f0fe8183cee59ec",
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
    "name": "Emma Taylor",
    "email": {
      "value": "sam.6508@example.com"
    },
    "id": "cust_dd3645c0633c40bc9cba1efe0c165e5d",
    "phone_number": "+917075704626"
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
        "value": "Ava"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3182 Pine Ln"
      },
      "line2": {
        "value": "5108 Lake Ave"
      },
      "line3": {
        "value": "1829 Sunset Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "17297"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9945@sandbox.example.com"
      },
      "phone_number": {
        "value": "1942237986"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION"
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
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:18 GMT
x-request-id: setup_recurring_setup_recurring_req

Response contents:
{
  "connectorRecurringPaymentId": "1cn21540r5461020k",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "577",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:18 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f34348588238b",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f34348588238b-5c814acccf68614e-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830086-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384498.800796,VS0,VE991"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "1cn21540r5461020k"
    }
  },
  "merchantRecurringPaymentId": "1cn21540r5461020k",
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
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v3/vault/payment-tokens/\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***",\"via\":\"HyperSwitch\",\"Prefer\":\"return=representation\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"PayPal-Request-Id\":\"mrpi_ae4ef17307ba43f39f0fe8183cee59ec\"},\"body\":{\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"3182 Pine Ln\",\"postal_code\":\"17297\",\"country_code\":\"US\",\"admin_area_2\":\"Chicago\"},\"expiry\":\"2030-08\",\"name\":\"Ava Johnson\",\"number\":\"4111111111111111\"}}}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
