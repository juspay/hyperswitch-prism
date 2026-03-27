# Connector `paypal` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
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
date: Fri, 13 Mar 2026 06:48:00 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29566",
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: paypal" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_fb3da7c00cc148e4b3741e678a817568",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "riley.7096@testmail.io"
    },
    "id": "cust_d2ee0cd2efa940da910eb79035f6cb63",
    "phone_number": "+919090320686"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29566"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "6764 Pine Blvd"
      },
      "line2": {
        "value": "7882 Market Blvd"
      },
      "line3": {
        "value": "1658 Sunset Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45543"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1098@testmail.io"
      },
      "phone_number": {
        "value": "6198922581"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2896 Main Ave"
      },
      "line2": {
        "value": "2864 Market Dr"
      },
      "line3": {
        "value": "8138 Lake Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32444"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1539@example.com"
      },
      "phone_number": {
        "value": "8409792522"
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
  "description": "No3DS auto capture card payment (credit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:03 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_fb3da7c00cc148e4b3741e678a817568",
  "connectorTransactionId": "6CU73366EX312824M",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2392",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:03 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f487981eada02",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f487981eada02-46a3ef616283f65d-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880060-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384481.075275,VS0,VE2268"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29566"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"6CU73366EX312824M\",\"intent\":\"CAPTURE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Liam Miller\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"CREDIT\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"tax_total\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_fb3da7c00cc148e4b3741e678a817568\",\"invoice_id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_fb3da7c00cc148e4b3741e678a817568\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Emma\"},\"address\":{\"address_line_1\":\"6764 Pine Blvd\",\"admin_area_2\":\"Austin\",\"postal_code\":\"45543\",\"country_code\":\"US\"}},\"payments\":{\"captures\":[{\"id\":\"1SP84550HG720313W\",\"status\":\"COMPLETED\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/1SP84550HG720313W\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/1SP84550HG720313W/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/6CU73366EX312824M\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:48:03Z\",\"update_time\":\"2026-03-13T06:48:03Z\",\"network_transaction_reference\":{\"id\":\"099571837410097\",\"network\":\"VISA\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"}}]}}],\"create_time\":\"2026-03-13T06:48:03Z\",\"update_time\":\"2026-03-13T06:48:03Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/6CU73366EX312824M\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"Prefer\":\"return=representation\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\",\"PayPal-Request-Id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":{\"intent\":\"CAPTURE\",\"purchase_units\":[{\"reference_id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"invoice_id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"6764 Pine Blvd\",\"postal_code\":\"45543\",\"country_code\":\"US\",\"admin_area_2\":\"Austin\"},\"name\":{\"full_name\":\"Emma\"}},\"items\":[{\"name\":\"Payment for invoice mti_fb3da7c00cc148e4b3741e678a817568\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"2896 Main Ave\",\"postal_code\":\"32444\",\"country_code\":\"US\",\"admin_area_2\":\"San Francisco\"},\"expiry\":\"2030-08\",\"name\":\"Liam Miller\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"1SP84550HG720313W\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "6CU73366EX312824M",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29566"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"1SP84550HG720313W\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:04 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "connectorTransactionId": "6CU73366EX312824M",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json;charset=UTF-8",
    "date": "Fri, 13 Mar 2026 06:48:03 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f721238ccd965",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f721238ccd965-ceccc875522137b3-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880088-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384484.545502,VS0,VE453"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29566"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"1SP84550HG720313W\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_fb3da7c00cc148e4b3741e678a817568\",\"status\":\"COMPLETED\",\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"},\"supplementary_data\":{\"related_ids\":{\"order_id\":\"6CU73366EX312824M\"}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"create_time\":\"2026-03-13T06:48:02Z\",\"update_time\":\"2026-03-13T06:48:02Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/1SP84550HG720313W\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/1SP84550HG720313W/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/6CU73366EX312824M\",\"rel\":\"up\",\"method\":\"GET\"}],\"network_transaction_reference\":{\"id\":\"099571837410097\",\"network\":\"VISA\"}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/captures/1SP84550HG720313W\",\"method\":\"GET\",\"headers\":{\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"PayPal-Request-Id\":\"6CU73366EX312824M\",\"Prefer\":\"return=representation\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
