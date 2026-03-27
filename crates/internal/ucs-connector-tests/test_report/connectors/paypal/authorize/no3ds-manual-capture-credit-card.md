# Connector `paypal` / Suite `authorize` / Scenario `no3ds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
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
date: Fri, 13 Mar 2026 06:46:53 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29633",
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
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_6b7424609fd142d1aadbdccd83618710",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
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
        "value": "Liam Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "riley.4794@sandbox.example.com"
    },
    "id": "cust_c464b3f3bc0f4f13943ca4b823175cc2",
    "phone_number": "+448204321807"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29633"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9390 Sunset Dr"
      },
      "line2": {
        "value": "8579 Lake St"
      },
      "line3": {
        "value": "257 Lake St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "28191"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1954@sandbox.example.com"
      },
      "phone_number": {
        "value": "8518871175"
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
        "value": "9239 Oak Rd"
      },
      "line2": {
        "value": "2481 Lake Rd"
      },
      "line3": {
        "value": "4156 Lake Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "98244"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5578@testmail.io"
      },
      "phone_number": {
        "value": "1515946864"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "No3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
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
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:04 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_6b7424609fd142d1aadbdccd83618710",
  "connectorTransactionId": "8GW62056DJ484615F",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2518",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:04 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f615819a5cf63",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f615819a5cf63-96074e78df83caba-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830022-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384422.870429,VS0,VE2135"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29633"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"8GW62056DJ484615F\",\"intent\":\"AUTHORIZE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Emma Miller\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"UNKNOWN\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_6b7424609fd142d1aadbdccd83618710\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_6b7424609fd142d1aadbdccd83618710\",\"invoice_id\":\"mti_6b7424609fd142d1aadbdccd83618710\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_6b7424609fd142d1aadbdccd83618710\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Ava\"},\"address\":{\"address_line_1\":\"9390 Sunset Dr\",\"admin_area_2\":\"Chicago\",\"admin_area_1\":\"XX\",\"postal_code\":\"28191\",\"country_code\":\"US\"}},\"payments\":{\"authorizations\":[{\"status\":\"CREATED\",\"id\":\"9U681043FY616771K\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"invoice_id\":\"mti_6b7424609fd142d1aadbdccd83618710\",\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"expiration_time\":\"2026-04-11T06:47:03Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/9U681043FY616771K\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/9U681043FY616771K/capture\",\"rel\":\"capture\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/9U681043FY616771K/void\",\"rel\":\"void\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/9U681043FY616771K/reauthorize\",\"rel\":\"reauthorize\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/8GW62056DJ484615F\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:03Z\",\"update_time\":\"2026-03-13T06:47:03Z\",\"network_transaction_reference\":{\"id\":\"853480467454748\",\"network\":\"VISA\"}}]}}],\"create_time\":\"2026-03-13T06:47:03Z\",\"update_time\":\"2026-03-13T06:47:03Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/8GW62056DJ484615F\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"Prefer\":\"return=representation\",\"PayPal-Request-Id\":\"mti_6b7424609fd142d1aadbdccd83618710\",\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\"},\"body\":{\"intent\":\"AUTHORIZE\",\"purchase_units\":[{\"reference_id\":\"mti_6b7424609fd142d1aadbdccd83618710\",\"invoice_id\":\"mti_6b7424609fd142d1aadbdccd83618710\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"9390 Sunset Dr\",\"postal_code\":\"28191\",\"country_code\":\"US\",\"admin_area_2\":\"Chicago\"},\"name\":{\"full_name\":\"Ava\"}},\"items\":[{\"name\":\"Payment for invoice mti_6b7424609fd142d1aadbdccd83618710\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"9239 Oak Rd\",\"postal_code\":\"98244\",\"country_code\":\"US\",\"admin_area_2\":\"Los Angeles\"},\"expiry\":\"2030-08\",\"name\":\"Emma Miller\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"9U681043FY616771K\",\"capture_id\":null,\"incremental_authorization_id\":\"9U681043FY616771K\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
