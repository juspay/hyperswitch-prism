# Connector `paypal` / Suite `authorize` / Scenario `no3ds_auto_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
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
  -H "x-request-id: authorize_no3ds_auto_capture_debit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_fd9ade7918c540a4a705b0c124b782c8",
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
        "value": "Ava Miller"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Smith",
    "email": {
      "value": "riley.5838@testmail.io"
    },
    "id": "cust_3874773c5b0244e7961b6be65382d6d7",
    "phone_number": "+917544774046"
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
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "220 Sunset Rd"
      },
      "line2": {
        "value": "6413 Oak Dr"
      },
      "line3": {
        "value": "5026 Lake Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71069"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3690@sandbox.example.com"
      },
      "phone_number": {
        "value": "5777037983"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4884 Main Ln"
      },
      "line2": {
        "value": "3044 Sunset St"
      },
      "line3": {
        "value": "5108 Lake Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "86855"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1190@example.com"
      },
      "phone_number": {
        "value": "3837131789"
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
  "description": "No3DS auto capture card payment (debit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:58 GMT
x-request-id: authorize_no3ds_auto_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "mti_fd9ade7918c540a4a705b0c124b782c8",
  "connectorTransactionId": "4U407402KJ1586210",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2397",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:46:58 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f7327929c9e70",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f7327929c9e70-256851d717cb0ed4-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880041-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384416.087922,VS0,VE2641"
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
    "value": "{\"id\":\"4U407402KJ1586210\",\"intent\":\"CAPTURE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Ava Taylor\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"CREDIT\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_fd9ade7918c540a4a705b0c124b782c8\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"tax_total\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_fd9ade7918c540a4a705b0c124b782c8\",\"invoice_id\":\"mti_fd9ade7918c540a4a705b0c124b782c8\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_fd9ade7918c540a4a705b0c124b782c8\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Liam\"},\"address\":{\"address_line_1\":\"220 Sunset Rd\",\"admin_area_2\":\"San Francisco\",\"postal_code\":\"71069\",\"country_code\":\"US\"}},\"payments\":{\"captures\":[{\"id\":\"6MT74241HU339682D\",\"status\":\"COMPLETED\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_fd9ade7918c540a4a705b0c124b782c8\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/6MT74241HU339682D\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/6MT74241HU339682D/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/4U407402KJ1586210\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:46:58Z\",\"update_time\":\"2026-03-13T06:46:58Z\",\"network_transaction_reference\":{\"id\":\"614316410675043\",\"network\":\"VISA\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"}}]}}],\"create_time\":\"2026-03-13T06:46:58Z\",\"update_time\":\"2026-03-13T06:46:58Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/4U407402KJ1586210\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Request-Id\":\"mti_fd9ade7918c540a4a705b0c124b782c8\",\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Prefer\":\"return=representation\"},\"body\":{\"intent\":\"CAPTURE\",\"purchase_units\":[{\"reference_id\":\"mti_fd9ade7918c540a4a705b0c124b782c8\",\"invoice_id\":\"mti_fd9ade7918c540a4a705b0c124b782c8\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"220 Sunset Rd\",\"postal_code\":\"71069\",\"country_code\":\"US\",\"admin_area_2\":\"San Francisco\"},\"name\":{\"full_name\":\"Liam\"}},\"items\":[{\"name\":\"Payment for invoice mti_fd9ade7918c540a4a705b0c124b782c8\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"4884 Main Ln\",\"postal_code\":\"86855\",\"country_code\":\"US\",\"admin_area_2\":\"New York\"},\"expiry\":\"2030-08\",\"name\":\"Ava Taylor\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"6MT74241HU339682D\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
