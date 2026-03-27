# Connector `paypal` / Suite `void` / Scenario `void_authorized_payment`

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
date: Fri, 13 Mar 2026 06:47:24 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29602",
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
  "merchant_transaction_id": "mti_89d4e663cf0b4d6baf15ac3339088e91",
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
    "name": "Mia Brown",
    "email": {
      "value": "jordan.8256@sandbox.example.com"
    },
    "id": "cust_d86e3a25da5749a8b1bfb2a5a9bc9150",
    "phone_number": "+918478207639"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29602"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6870 Main Ln"
      },
      "line2": {
        "value": "1189 Oak St"
      },
      "line3": {
        "value": "3645 Oak Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "26113"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8803@sandbox.example.com"
      },
      "phone_number": {
        "value": "6005413786"
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
        "value": "4599 Market Dr"
      },
      "line2": {
        "value": "2162 Sunset Blvd"
      },
      "line3": {
        "value": "5758 Lake Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18403"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.7168@example.com"
      },
      "phone_number": {
        "value": "5183122293"
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
date: Fri, 13 Mar 2026 06:47:27 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_89d4e663cf0b4d6baf15ac3339088e91",
  "connectorTransactionId": "4RC10267GT597441B",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2516",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:27 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f66028638c080",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f66028638c080-5058cd171c8954d4-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830053-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384445.749386,VS0,VE2381"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29602"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"4RC10267GT597441B\",\"intent\":\"AUTHORIZE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Emma Miller\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"UNKNOWN\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_89d4e663cf0b4d6baf15ac3339088e91\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_89d4e663cf0b4d6baf15ac3339088e91\",\"invoice_id\":\"mti_89d4e663cf0b4d6baf15ac3339088e91\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_89d4e663cf0b4d6baf15ac3339088e91\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Mia\"},\"address\":{\"address_line_1\":\"6870 Main Ln\",\"admin_area_2\":\"Chicago\",\"admin_area_1\":\"XX\",\"postal_code\":\"26113\",\"country_code\":\"US\"}},\"payments\":{\"authorizations\":[{\"status\":\"CREATED\",\"id\":\"83J907759V454312A\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"invoice_id\":\"mti_89d4e663cf0b4d6baf15ac3339088e91\",\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"expiration_time\":\"2026-04-11T06:47:26Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/83J907759V454312A\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/83J907759V454312A/capture\",\"rel\":\"capture\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/83J907759V454312A/void\",\"rel\":\"void\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/authorizations/83J907759V454312A/reauthorize\",\"rel\":\"reauthorize\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/4RC10267GT597441B\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:26Z\",\"update_time\":\"2026-03-13T06:47:26Z\",\"network_transaction_reference\":{\"id\":\"345587572485673\",\"network\":\"VISA\"}}]}}],\"create_time\":\"2026-03-13T06:47:26Z\",\"update_time\":\"2026-03-13T06:47:26Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/4RC10267GT597441B\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Request-Id\":\"mti_89d4e663cf0b4d6baf15ac3339088e91\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"via\":\"HyperSwitch\",\"Prefer\":\"return=representation\",\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":{\"intent\":\"AUTHORIZE\",\"purchase_units\":[{\"reference_id\":\"mti_89d4e663cf0b4d6baf15ac3339088e91\",\"invoice_id\":\"mti_89d4e663cf0b4d6baf15ac3339088e91\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"6870 Main Ln\",\"postal_code\":\"26113\",\"country_code\":\"US\",\"admin_area_2\":\"Chicago\"},\"name\":{\"full_name\":\"Mia\"}},\"items\":[{\"name\":\"Payment for invoice mti_89d4e663cf0b4d6baf15ac3339088e91\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"4599 Market Dr\",\"postal_code\":\"18403\",\"country_code\":\"US\",\"admin_area_2\":\"Los Angeles\"},\"expiry\":\"2030-08\",\"name\":\"Emma Miller\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"83J907759V454312A\",\"capture_id\":null,\"incremental_authorization_id\":\"83J907759V454312A\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-request-id: void_void_authorized_payment_req" \
  -H "x-connector-request-reference-id: void_void_authorized_payment_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "4RC10267GT597441B",
  "merchant_void_id": "mvi_11fe18545acb46318f19d716c7bc3ec0",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29602"
    }
  },
  "cancellation_reason": "requested_by_customer",
  "connector_feature_data": {
    "value": "{\"authorize_id\":\"83J907759V454312A\",\"capture_id\":null,\"incremental_authorization_id\":\"83J907759V454312A\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
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
x-connector-request-reference-id: void_void_authorized_payment_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: void_void_authorized_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:28 GMT
x-request-id: void_void_authorized_payment_req

Response contents:
{
  "connectorTransactionId": "83J907759V454312A",
  "status": "VOIDED",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:28 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f162731f7bcd6",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f162731f7bcd6-df2c45c7582d11aa-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830073-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384447.351506,VS0,VE735"
  },
  "merchantVoidId": "mti_89d4e663cf0b4d6baf15ac3339088e91",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29602"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/authorizations/83J907759V454312A/void\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"PayPal-Request-Id\":\"mvi_11fe18545acb46318f19d716c7bc3ec0\",\"Prefer\":\"return=representation\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/json\"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
