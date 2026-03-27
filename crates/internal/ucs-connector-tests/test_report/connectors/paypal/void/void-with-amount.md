# Connector `paypal` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
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
date: Fri, 13 Mar 2026 06:47:29 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29598",
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
<summary>2. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

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
  "merchant_transaction_id": "mti_7327c733df5c4b28bca52e89fd0239cd",
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
        "value": "Ethan Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "riley.4541@testmail.io"
    },
    "id": "cust_12145f73a7fc460f9fbd6eb2ef3198fc",
    "phone_number": "+443950296214"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29598"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4521 Sunset Dr"
      },
      "line2": {
        "value": "1839 Lake Blvd"
      },
      "line3": {
        "value": "3711 Market Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83642"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6119@example.com"
      },
      "phone_number": {
        "value": "2816974303"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "980 Market Blvd"
      },
      "line2": {
        "value": "3393 Market Ln"
      },
      "line3": {
        "value": "4278 Lake Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99402"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8888@sandbox.example.com"
      },
      "phone_number": {
        "value": "1468341420"
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
<summary>Show Dependency Response (masked)</summary>

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
date: Fri, 13 Mar 2026 06:47:32 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_7327c733df5c4b28bca52e89fd0239cd",
  "connectorTransactionId": "7FP29083F5313574Y",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2519",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:32 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f381260ee3583",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f381260ee3583-25705d8467edce31-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830099-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384449.414393,VS0,VE2639"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29598"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"7FP29083F5313574Y\",\"intent\":\"AUTHORIZE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Mia Taylor\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"UNKNOWN\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_7327c733df5c4b28bca52e89fd0239cd\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_7327c733df5c4b28bca52e89fd0239cd\",\"invoice_id\":\"mti_7327c733df5c4b28bca52e89fd0239cd\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_7327c733df5c4b28bca52e89fd0239cd\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Ethan\"},\"address\":{\"address_line_1\":\"4521 Sunset Dr\",\"admin_area_2\":\"Chicago\",\"admin_area_1\":\"XX\",\"postal_code\":\"83642\",\"country_code\":\"US\"}},\"payments\":{\"authorizations\":[{\"status\":\"CREATED\",\"id\":\"1UL07906VX739494J\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"invoice_id\":\"mti_7327c733df5c4b28bca52e89fd0239cd\",\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"expiration_time\":\"2026-04-11T06:47:31Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/1UL07906VX739494J\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/1UL07906VX739494J/capture\",\"rel\":\"capture\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/1UL07906VX739494J/void\",\"rel\":\"void\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/1UL07906VX739494J/reauthorize\",\"rel\":\"reauthorize\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7FP29083F5313574Y\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:31Z\",\"update_time\":\"2026-03-13T06:47:31Z\",\"network_transaction_reference\":{\"id\":\"897699851467304\",\"network\":\"VISA\"}}]}}],\"create_time\":\"2026-03-13T06:47:31Z\",\"update_time\":\"2026-03-13T06:47:31Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7FP29083F5313574Y\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"via\":\"HyperSwitch\",\"PayPal-Request-Id\":\"mti_7327c733df5c4b28bca52e89fd0239cd\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\"},\"body\":{\"intent\":\"AUTHORIZE\",\"purchase_units\":[{\"reference_id\":\"mti_7327c733df5c4b28bca52e89fd0239cd\",\"invoice_id\":\"mti_7327c733df5c4b28bca52e89fd0239cd\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"4521 Sunset Dr\",\"postal_code\":\"83642\",\"country_code\":\"US\",\"admin_area_2\":\"Chicago\"},\"name\":{\"full_name\":\"Ethan\"}},\"items\":[{\"name\":\"Payment for invoice mti_7327c733df5c4b28bca52e89fd0239cd\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"980 Market Blvd\",\"postal_code\":\"99402\",\"country_code\":\"US\",\"admin_area_2\":\"Austin\"},\"expiry\":\"2030-08\",\"name\":\"Mia Taylor\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"1UL07906VX739494J\",\"capture_id\":null,\"incremental_authorization_id\":\"1UL07906VX739494J\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-connector: paypal" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: void_void_with_amount_req" \
  -H "x-connector-request-reference-id: void_void_with_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "7FP29083F5313574Y",
  "merchant_void_id": "mvi_f18a615133a74787a05773cf29e7d9ff",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_775984",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29598"
    }
  },
  "cancellation_reason": "requested_by_customer",
  "connector_feature_data": {
    "value": "{\"authorize_id\":\"1UL07906VX739494J\",\"capture_id\":null,\"incremental_authorization_id\":\"1UL07906VX739494J\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Cancel an authorized payment before capture. Releases held funds back to
// customer, typically used when orders are cancelled or abandoned.
rpc Void ( .types.PaymentServiceVoidRequest ) returns ( .types.PaymentServiceVoidResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: void_void_with_amount_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: void_void_with_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:33 GMT
x-request-id: void_void_with_amount_req

Response contents:
{
  "connectorTransactionId": "1UL07906VX739494J",
  "status": "VOIDED",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:33 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f2626022e81f1",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f2626022e81f1-003d6e7d6aabef9d-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880038-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384452.254091,VS0,VE759"
  },
  "merchantVoidId": "mti_7327c733df5c4b28bca52e89fd0239cd",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29598"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/authorizations/1UL07906VX739494J/void\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"PayPal-Request-Id\":\"mvi_f18a615133a74787a05773cf29e7d9ff\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
