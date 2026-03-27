# Connector `paypal` / Suite `refund_sync` / Scenario `refund_sync_with_reason`

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
date: Fri, 13 Mar 2026 06:48:11 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29555",
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
  "merchant_transaction_id": "mti_8a7aed3efe0a4660968fef77ec0ef040",
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
        "value": "Ava Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Brown",
    "email": {
      "value": "sam.5526@testmail.io"
    },
    "id": "cust_476e05136ef34befbb62dcc3fd3760a6",
    "phone_number": "+915977490673"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29555"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "619 Pine Rd"
      },
      "line2": {
        "value": "4248 Oak Ln"
      },
      "line3": {
        "value": "4951 Oak St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "17723"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3801@example.com"
      },
      "phone_number": {
        "value": "1471354242"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "6960 Lake Ave"
      },
      "line2": {
        "value": "476 Main Blvd"
      },
      "line3": {
        "value": "8589 Sunset Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "37061"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1851@example.com"
      },
      "phone_number": {
        "value": "1302483685"
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
date: Fri, 13 Mar 2026 06:48:13 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_8a7aed3efe0a4660968fef77ec0ef040",
  "connectorTransactionId": "83667241NH279522S",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2389",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:13 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f3831488f5604",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f3831488f5604-0184b4ec54491665-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880061-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384491.359730,VS0,VE2065"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29555"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"83667241NH279522S\",\"intent\":\"CAPTURE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Mia Johnson\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"CREDIT\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"tax_total\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_8a7aed3efe0a4660968fef77ec0ef040\",\"invoice_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_8a7aed3efe0a4660968fef77ec0ef040\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Ava\"},\"address\":{\"address_line_1\":\"619 Pine Rd\",\"admin_area_2\":\"Chicago\",\"postal_code\":\"17723\",\"country_code\":\"US\"}},\"payments\":{\"captures\":[{\"id\":\"9DW8065711210961C\",\"status\":\"COMPLETED\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/9DW8065711210961C\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/9DW8065711210961C/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/83667241NH279522S\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:48:13Z\",\"update_time\":\"2026-03-13T06:48:13Z\",\"network_transaction_reference\":{\"id\":\"635824445739918\",\"network\":\"VISA\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"}}]}}],\"create_time\":\"2026-03-13T06:48:13Z\",\"update_time\":\"2026-03-13T06:48:13Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/83667241NH279522S\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\",\"Prefer\":\"return=representation\",\"PayPal-Request-Id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\"},\"body\":{\"intent\":\"CAPTURE\",\"purchase_units\":[{\"reference_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"invoice_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"619 Pine Rd\",\"postal_code\":\"17723\",\"country_code\":\"US\",\"admin_area_2\":\"Chicago\"},\"name\":{\"full_name\":\"Ava\"}},\"items\":[{\"name\":\"Payment for invoice mti_8a7aed3efe0a4660968fef77ec0ef040\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"6960 Lake Ave\",\"postal_code\":\"37061\",\"country_code\":\"US\",\"admin_area_2\":\"Seattle\"},\"expiry\":\"2030-08\",\"name\":\"Mia Johnson\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"9DW8065711210961C\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
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
  "merchant_refund_id": "mri_9bc376bf6a03471a9119752adc6165da",
  "connector_transaction_id": "83667241NH279522S",
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
      "expires_in_seconds": "29555"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"9DW8065711210961C\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
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
date: Fri, 13 Mar 2026 06:48:15 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "19E08322XA2972132",
  "status": "REFUND_SUCCESS",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "710",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:15 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f37522777b3f1",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f37522777b3f1-45d65248ccdd3713-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830095-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384494.639438,VS0,VE1942"
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"19E08322XA2972132\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"seller_payable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"paypal_fee\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"total_refunded_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"status\":\"COMPLETED\",\"create_time\":\"2026-03-12T23:48:14-07:00\",\"update_time\":\"2026-03-12T23:48:14-07:00\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/refunds/19E08322XA2972132\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/9DW8065711210961C\",\"rel\":\"up\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/captures/9DW8065711210961C/refund\",\"method\":\"POST\",\"headers\":{\"PayPal-Request-Id\":\"mri_9bc376bf6a03471a9119752adc6165da\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Prefer\":\"return=representation\",\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":{\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}}}"
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
  -H "x-request-id: refund_sync_refund_sync_with_reason_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "83667241NH279522S",
  "refund_id": "19E08322XA2972132",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29555"
    }
  },
  "refund_reason": "customer_requested",
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"9DW8065711210961C\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
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
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:16 GMT
x-request-id: refund_sync_refund_sync_with_reason_req

Response contents:
{
  "merchantRefundId": "19E08322XA2972132",
  "connectorRefundId": "19E08322XA2972132",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:16 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f7233408f1f15",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f7233408f1f15-fac0ed1855d1eb1b-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880084-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384496.764374,VS0,VE935"
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"19E08322XA2972132\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"seller_payable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"paypal_fee\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"total_refunded_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_8a7aed3efe0a4660968fef77ec0ef040\",\"status\":\"COMPLETED\",\"create_time\":\"2026-03-12T23:48:14-07:00\",\"update_time\":\"2026-03-12T23:48:14-07:00\",\"payer\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/refunds/19E08322XA2972132\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/9DW8065711210961C\",\"rel\":\"up\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/refunds/19E08322XA2972132\",\"method\":\"GET\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Prefer\":\"return=representation\",\"Content-Type\":\"application/json\",\"PayPal-Request-Id\":\"\",\"via\":\"HyperSwitch\"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
