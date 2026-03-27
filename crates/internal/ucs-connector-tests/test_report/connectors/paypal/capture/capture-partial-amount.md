# Connector `paypal` / Suite `capture` / Scenario `capture_partial_amount`

- Service: `PaymentService/Capture`
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
date: Fri, 13 Mar 2026 06:47:13 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29613",
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
  "merchant_transaction_id": "mti_49e95dadd214486583525f476a5f9935",
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
        "value": "Ava Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "jordan.2271@testmail.io"
    },
    "id": "cust_3d09c808c6e04d2988801e14826fc43f",
    "phone_number": "+915363675722"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29613"
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
        "value": "9704 Sunset Rd"
      },
      "line2": {
        "value": "6744 Pine St"
      },
      "line3": {
        "value": "7816 Lake Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "48743"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2569@example.com"
      },
      "phone_number": {
        "value": "1945992268"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5940 Pine Blvd"
      },
      "line2": {
        "value": "6994 Lake Rd"
      },
      "line3": {
        "value": "2529 Sunset Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "13052"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4873@sandbox.example.com"
      },
      "phone_number": {
        "value": "1286956118"
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
date: Fri, 13 Mar 2026 06:47:16 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_49e95dadd214486583525f476a5f9935",
  "connectorTransactionId": "9CU55560TL7965517",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2518",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:16 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f9257466c2d78",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f9257466c2d78-0f16c7ecd4a5a98a-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880069-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384434.728748,VS0,VE2580"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29613"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"9CU55560TL7965517\",\"intent\":\"AUTHORIZE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Mia Brown\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"UNKNOWN\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_49e95dadd214486583525f476a5f9935\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_49e95dadd214486583525f476a5f9935\",\"invoice_id\":\"mti_49e95dadd214486583525f476a5f9935\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_49e95dadd214486583525f476a5f9935\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Noah\"},\"address\":{\"address_line_1\":\"9704 Sunset Rd\",\"admin_area_2\":\"New York\",\"admin_area_1\":\"XX\",\"postal_code\":\"48743\",\"country_code\":\"US\"}},\"payments\":{\"authorizations\":[{\"status\":\"CREATED\",\"id\":\"96P698215K734160F\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"invoice_id\":\"mti_49e95dadd214486583525f476a5f9935\",\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"expiration_time\":\"2026-04-11T06:47:15Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/96P698215K734160F\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/96P698215K734160F/capture\",\"rel\":\"capture\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/96P698215K734160F/void\",\"rel\":\"void\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/96P698215K734160F/reauthorize\",\"rel\":\"reauthorize\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/9CU55560TL7965517\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:15Z\",\"update_time\":\"2026-03-13T06:47:15Z\",\"network_transaction_reference\":{\"id\":\"091304287620028\",\"network\":\"VISA\"}}]}}],\"create_time\":\"2026-03-13T06:47:15Z\",\"update_time\":\"2026-03-13T06:47:15Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/9CU55560TL7965517\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Request-Id\":\"mti_49e95dadd214486583525f476a5f9935\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":{\"intent\":\"AUTHORIZE\",\"purchase_units\":[{\"reference_id\":\"mti_49e95dadd214486583525f476a5f9935\",\"invoice_id\":\"mti_49e95dadd214486583525f476a5f9935\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"9704 Sunset Rd\",\"postal_code\":\"48743\",\"country_code\":\"US\",\"admin_area_2\":\"New York\"},\"name\":{\"full_name\":\"Noah\"}},\"items\":[{\"name\":\"Payment for invoice mti_49e95dadd214486583525f476a5f9935\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"5940 Pine Blvd\",\"postal_code\":\"13052\",\"country_code\":\"US\",\"admin_area_2\":\"Austin\"},\"expiry\":\"2030-08\",\"name\":\"Mia Brown\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"96P698215K734160F\",\"capture_id\":null,\"incremental_authorization_id\":\"96P698215K734160F\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-request-id: capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_partial_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "9CU55560TL7965517",
  "amount_to_capture": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_07aface881a34a07985a7e1b1b4d2701",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29613"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":\"96P698215K734160F\",\"capture_id\":null,\"incremental_authorization_id\":\"96P698215K734160F\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: capture_capture_partial_amount_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:18 GMT
x-request-id: capture_capture_partial_amount_req

Response contents:
{
  "connectorTransactionId": "9CU55560TL7965517",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "925",
    "content-type": "application/json;charset=UTF-8",
    "date": "Fri, 13 Mar 2026 06:47:18 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f4618824bfb7c",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f4618824bfb7c-664a8d970da74875-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830062-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384436.472736,VS0,VE2185"
  },
  "merchantCaptureId": "mti_49e95dadd214486583525f476a5f9935",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29613"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/authorizations/96P698215K734160F/capture\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"PayPal-Request-Id\":\"mci_07aface881a34a07985a7e1b1b4d2701\",\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Prefer\":\"return=representation\",\"via\":\"HyperSwitch\"},\"body\":{\"amount\":{\"currency_code\":\"USD\",\"value\":\"30.00\"},\"final_capture\":true}}"
  },
  "capturedAmount": "3000",
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"96P698215K734160F\",\"capture_id\":\"3H956669DM410410J\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
