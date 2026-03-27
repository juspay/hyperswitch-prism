# Connector `paypal` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
date: Fri, 13 Mar 2026 06:47:19 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29607",
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
  "merchant_transaction_id": "mti_463324bba2fb498fb2d3b5e08d40c495",
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
        "value": "Noah Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Smith",
    "email": {
      "value": "casey.6667@testmail.io"
    },
    "id": "cust_28cec5669f0f411c8e0e0f4964c32847",
    "phone_number": "+443770147899"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29607"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9040 Oak St"
      },
      "line2": {
        "value": "9274 Lake St"
      },
      "line3": {
        "value": "9940 Pine St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "51663"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1068@example.com"
      },
      "phone_number": {
        "value": "3273106958"
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
        "value": "1374 Market Dr"
      },
      "line2": {
        "value": "9495 Lake Ln"
      },
      "line3": {
        "value": "9284 Main St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30385"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2536@testmail.io"
      },
      "phone_number": {
        "value": "5075830745"
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
date: Fri, 13 Mar 2026 06:47:22 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_463324bba2fb498fb2d3b5e08d40c495",
  "connectorTransactionId": "7RX66603FM938094C",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2520",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:22 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f505267ce080f",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f505267ce080f-bee9f49766b982af-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830062-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384440.891499,VS0,VE2169"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29607"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"7RX66603FM938094C\",\"intent\":\"AUTHORIZE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Mia Taylor\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"UNKNOWN\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_463324bba2fb498fb2d3b5e08d40c495\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_463324bba2fb498fb2d3b5e08d40c495\",\"invoice_id\":\"mti_463324bba2fb498fb2d3b5e08d40c495\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_463324bba2fb498fb2d3b5e08d40c495\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Mia\"},\"address\":{\"address_line_1\":\"9040 Oak St\",\"admin_area_2\":\"San Francisco\",\"admin_area_1\":\"XX\",\"postal_code\":\"51663\",\"country_code\":\"US\"}},\"payments\":{\"authorizations\":[{\"status\":\"CREATED\",\"id\":\"5B561600UW2825707\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"invoice_id\":\"mti_463324bba2fb498fb2d3b5e08d40c495\",\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"expiration_time\":\"2026-04-11T06:47:21Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/5B561600UW2825707\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/5B561600UW2825707/capture\",\"rel\":\"capture\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/5B561600UW2825707/void\",\"rel\":\"void\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/5B561600UW2825707/reauthorize\",\"rel\":\"reauthorize\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7RX66603FM938094C\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:21Z\",\"update_time\":\"2026-03-13T06:47:21Z\",\"network_transaction_reference\":{\"id\":\"346208391641905\",\"network\":\"VISA\"}}]}}],\"create_time\":\"2026-03-13T06:47:21Z\",\"update_time\":\"2026-03-13T06:47:21Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7RX66603FM938094C\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\",\"Prefer\":\"return=representation\",\"PayPal-Request-Id\":\"mti_463324bba2fb498fb2d3b5e08d40c495\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":{\"intent\":\"AUTHORIZE\",\"purchase_units\":[{\"reference_id\":\"mti_463324bba2fb498fb2d3b5e08d40c495\",\"invoice_id\":\"mti_463324bba2fb498fb2d3b5e08d40c495\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"9040 Oak St\",\"postal_code\":\"51663\",\"country_code\":\"US\",\"admin_area_2\":\"San Francisco\"},\"name\":{\"full_name\":\"Mia\"}},\"items\":[{\"name\":\"Payment for invoice mti_463324bba2fb498fb2d3b5e08d40c495\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"1374 Market Dr\",\"postal_code\":\"30385\",\"country_code\":\"US\",\"admin_area_2\":\"Seattle\"},\"expiry\":\"2030-08\",\"name\":\"Mia Taylor\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"5B561600UW2825707\",\"capture_id\":null,\"incremental_authorization_id\":\"5B561600UW2825707\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "7RX66603FM938094C",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_542ba00381424e579044a9cb22bbad3f",
  "merchant_order_id": "gen_624873",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29607"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":\"5B561600UW2825707\",\"capture_id\":null,\"incremental_authorization_id\":\"5B561600UW2825707\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:23 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "7RX66603FM938094C",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "925",
    "content-type": "application/json;charset=UTF-8",
    "date": "Fri, 13 Mar 2026 06:47:23 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f924467a11eac",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f924467a11eac-a389514fdf9b4a74-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880097-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384442.247440,VS0,VE1723"
  },
  "merchantCaptureId": "mti_463324bba2fb498fb2d3b5e08d40c495",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29607"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/authorizations/5B561600UW2825707/capture\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Request-Id\":\"mci_542ba00381424e579044a9cb22bbad3f\",\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Prefer\":\"return=representation\"},\"body\":{\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true}}"
  },
  "capturedAmount": "6000",
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"5B561600UW2825707\",\"capture_id\":\"59H63203JH9834613\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
