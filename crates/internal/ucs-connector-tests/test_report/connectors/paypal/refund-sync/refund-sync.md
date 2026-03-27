# Connector `paypal` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
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
date: Fri, 13 Mar 2026 06:48:05 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29562",
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
  "merchant_transaction_id": "mti_b678a826e026410f99d1c46685128499",
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
        "value": "Noah Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "alex.4135@example.com"
    },
    "id": "cust_fc4d9a9923df4e0da35da231ee166d63",
    "phone_number": "+18804391904"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29562"
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
        "value": "5460 Market Rd"
      },
      "line2": {
        "value": "7096 Pine Blvd"
      },
      "line3": {
        "value": "738 Main Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "62882"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8893@sandbox.example.com"
      },
      "phone_number": {
        "value": "2801816288"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "4466 Main Ln"
      },
      "line2": {
        "value": "2488 Main Rd"
      },
      "line3": {
        "value": "4756 Pine Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "57241"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5148@testmail.io"
      },
      "phone_number": {
        "value": "9661917642"
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
date: Fri, 13 Mar 2026 06:48:07 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_b678a826e026410f99d1c46685128499",
  "connectorTransactionId": "7A9498376K362014B",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2391",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:07 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f365176fbb676",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f365176fbb676-84dcb66480d006c8-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830072-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384485.202954,VS0,VE2416"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29562"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"7A9498376K362014B\",\"intent\":\"CAPTURE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Emma Smith\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"CREDIT\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_b678a826e026410f99d1c46685128499\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"tax_total\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_b678a826e026410f99d1c46685128499\",\"invoice_id\":\"mti_b678a826e026410f99d1c46685128499\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_b678a826e026410f99d1c46685128499\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Mia\"},\"address\":{\"address_line_1\":\"5460 Market Rd\",\"admin_area_2\":\"Seattle\",\"postal_code\":\"62882\",\"country_code\":\"US\"}},\"payments\":{\"captures\":[{\"id\":\"2HY09398FS7935707\",\"status\":\"COMPLETED\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_b678a826e026410f99d1c46685128499\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/2HY09398FS7935707\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/2HY09398FS7935707/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7A9498376K362014B\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:48:07Z\",\"update_time\":\"2026-03-13T06:48:07Z\",\"network_transaction_reference\":{\"id\":\"552856478862729\",\"network\":\"VISA\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"}}]}}],\"create_time\":\"2026-03-13T06:48:07Z\",\"update_time\":\"2026-03-13T06:48:07Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/7A9498376K362014B\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"Prefer\":\"return=representation\",\"PayPal-Request-Id\":\"mti_b678a826e026410f99d1c46685128499\",\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Content-Type\":\"application/json\"},\"body\":{\"intent\":\"CAPTURE\",\"purchase_units\":[{\"reference_id\":\"mti_b678a826e026410f99d1c46685128499\",\"invoice_id\":\"mti_b678a826e026410f99d1c46685128499\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"5460 Market Rd\",\"postal_code\":\"62882\",\"country_code\":\"US\",\"admin_area_2\":\"Seattle\"},\"name\":{\"full_name\":\"Mia\"}},\"items\":[{\"name\":\"Payment for invoice mti_b678a826e026410f99d1c46685128499\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"4466 Main Ln\",\"postal_code\":\"57241\",\"country_code\":\"US\",\"admin_area_2\":\"Los Angeles\"},\"expiry\":\"2030-08\",\"name\":\"Emma Smith\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"2HY09398FS7935707\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>3. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: paypal" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_5d00b3699f00498fba0d920aaafb1b2b",
  "connector_transaction_id": "7A9498376K362014B",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29562"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"2HY09398FS7935707\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: refund_refund_full_amount_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:10 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "3CS01704L04009225",
  "status": "REFUND_SUCCESS",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "710",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:10 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f921812debe54",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f921812debe54-382e758708a1b7c4-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830066-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384488.786388,VS0,VE2305"
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"3CS01704L04009225\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"seller_payable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"paypal_fee\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"total_refunded_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_b678a826e026410f99d1c46685128499\",\"status\":\"COMPLETED\",\"create_time\":\"2026-03-12T23:48:08-07:00\",\"update_time\":\"2026-03-12T23:48:08-07:00\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/refunds/3CS01704L04009225\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/2HY09398FS7935707\",\"rel\":\"up\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/captures/2HY09398FS7935707/refund\",\"method\":\"POST\",\"headers\":{\"PayPal-Request-Id\":\"mri_5d00b3699f00498fba0d920aaafb1b2b\",\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\"},\"body\":{\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}}}"
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "7A9498376K362014B",
  "refund_id": "3CS01704L04009225",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29562"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"2HY09398FS7935707\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:10 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "merchantRefundId": "3CS01704L04009225",
  "connectorRefundId": "3CS01704L04009225",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:10 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f9552829ba1a0",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f9552829ba1a0-290aa966d4b00ef4-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830027-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384490.283872,VS0,VE412"
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"3CS01704L04009225\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"seller_payable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"paypal_fee\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"total_refunded_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_b678a826e026410f99d1c46685128499\",\"status\":\"COMPLETED\",\"create_time\":\"2026-03-12T23:48:08-07:00\",\"update_time\":\"2026-03-12T23:48:08-07:00\",\"payer\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/refunds/3CS01704L04009225\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/2HY09398FS7935707\",\"rel\":\"up\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/refunds/3CS01704L04009225\",\"method\":\"GET\",\"headers\":{\"PayPal-Request-Id\":\"\",\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\",\"Authorization\":\"Bearer ***MASKED***",\"via\":\"HyperSwitch\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
