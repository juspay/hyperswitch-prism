# Connector `paypal` / Suite `void` / Scenario `void_without_cancellation_reason`

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
date: Fri, 13 Mar 2026 06:47:33 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29593",
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
  "merchant_transaction_id": "mti_367fa2fdbf7b42129ab88a17b34554cc",
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
        "value": "Ethan Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Wilson",
    "email": {
      "value": "jordan.9221@example.com"
    },
    "id": "cust_51453bd8ee414443a468972c0b9bfa60",
    "phone_number": "+17235617659"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29593"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "380 Market Blvd"
      },
      "line2": {
        "value": "4229 Oak Dr"
      },
      "line3": {
        "value": "7949 Pine Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31483"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8989@sandbox.example.com"
      },
      "phone_number": {
        "value": "6859430365"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1179 Market Ln"
      },
      "line2": {
        "value": "9069 Lake Blvd"
      },
      "line3": {
        "value": "3385 Market Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53563"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4579@testmail.io"
      },
      "phone_number": {
        "value": "3222152531"
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
date: Fri, 13 Mar 2026 06:47:36 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_367fa2fdbf7b42129ab88a17b34554cc",
  "connectorTransactionId": "7BA28601TA0140722",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2520",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:36 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f7287582364ab",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f7287582364ab-a69fca1eb0fc79d4-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880020-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384454.181676,VS0,VE2514"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29593"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"7BA28601TA0140722\",\"intent\":\"AUTHORIZE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Noah Miller\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"UNKNOWN\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_367fa2fdbf7b42129ab88a17b34554cc\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_367fa2fdbf7b42129ab88a17b34554cc\",\"invoice_id\":\"mti_367fa2fdbf7b42129ab88a17b34554cc\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_367fa2fdbf7b42129ab88a17b34554cc\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Ethan\"},\"address\":{\"address_line_1\":\"380 Market Blvd\",\"admin_area_2\":\"Austin\",\"admin_area_1\":\"XX\",\"postal_code\":\"31483\",\"country_code\":\"US\"}},\"payments\":{\"authorizations\":[{\"status\":\"CREATED\",\"id\":\"8HH768166Y669220U\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"invoice_id\":\"mti_367fa2fdbf7b42129ab88a17b34554cc\",\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"expiration_time\":\"2026-04-11T06:47:35Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/8HH768166Y669220U\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/8HH768166Y669220U/capture\",\"rel\":\"capture\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/8HH768166Y669220U/void\",\"rel\":\"void\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/8HH768166Y669220U/reauthorize\",\"rel\":\"reauthorize\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7BA28601TA0140722\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:35Z\",\"update_time\":\"2026-03-13T06:47:35Z\",\"network_transaction_reference\":{\"id\":\"847469902069194\",\"network\":\"VISA\"}}]}}],\"create_time\":\"2026-03-13T06:47:35Z\",\"update_time\":\"2026-03-13T06:47:35Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7BA28601TA0140722\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Prefer\":\"return=representation\",\"PayPal-Request-Id\":\"mti_367fa2fdbf7b42129ab88a17b34554cc\",\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***",\"via\":\"HyperSwitch\"},\"body\":{\"intent\":\"AUTHORIZE\",\"purchase_units\":[{\"reference_id\":\"mti_367fa2fdbf7b42129ab88a17b34554cc\",\"invoice_id\":\"mti_367fa2fdbf7b42129ab88a17b34554cc\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"380 Market Blvd\",\"postal_code\":\"31483\",\"country_code\":\"US\",\"admin_area_2\":\"Austin\"},\"name\":{\"full_name\":\"Ethan\"}},\"items\":[{\"name\":\"Payment for invoice mti_367fa2fdbf7b42129ab88a17b34554cc\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"1179 Market Ln\",\"postal_code\":\"53563\",\"country_code\":\"US\",\"admin_area_2\":\"Seattle\"},\"expiry\":\"2030-08\",\"name\":\"Noah Miller\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"8HH768166Y669220U\",\"capture_id\":null,\"incremental_authorization_id\":\"8HH768166Y669220U\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-request-id: void_void_without_cancellation_reason_req" \
  -H "x-connector-request-reference-id: void_void_without_cancellation_reason_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "7BA28601TA0140722",
  "merchant_void_id": "mvi_2ad38ea665bd4839adc7e34246389d54",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29593"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":\"8HH768166Y669220U\",\"capture_id\":null,\"incremental_authorization_id\":\"8HH768166Y669220U\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
x-connector-request-reference-id: void_void_without_cancellation_reason_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: void_void_without_cancellation_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:38 GMT
x-request-id: void_void_without_cancellation_reason_req

Response contents:
{
  "connectorTransactionId": "8HH768166Y669220U",
  "status": "VOIDED",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:38 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f716142c3f945",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f716142c3f945-0e1edc0741df795d-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880098-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384457.875767,VS0,VE1299"
  },
  "merchantVoidId": "mti_367fa2fdbf7b42129ab88a17b34554cc",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29593"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/authorizations/8HH768166Y669220U/void\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***",\"Prefer\":\"return=representation\",\"PayPal-Request-Id\":\"mvi_2ad38ea665bd4839adc7e34246389d54\"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
