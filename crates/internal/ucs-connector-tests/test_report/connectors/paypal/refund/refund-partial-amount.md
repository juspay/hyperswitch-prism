# Connector `paypal` / Suite `refund` / Scenario `refund_partial_amount`

- Service: `PaymentService/Refund`
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
date: Fri, 13 Mar 2026 06:47:44 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29582",
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
  "merchant_transaction_id": "mti_f5722a99117a4799b923743f1d998fea",
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
        "value": "Noah Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Wilson",
    "email": {
      "value": "sam.6536@example.com"
    },
    "id": "cust_04bf4eeb56794964a8f1c5ddcefbde39",
    "phone_number": "+447976605482"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29582"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9231 Main Rd"
      },
      "line2": {
        "value": "129 Main Ln"
      },
      "line3": {
        "value": "9414 Market Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "72663"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5119@testmail.io"
      },
      "phone_number": {
        "value": "2617846660"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9821 Sunset Rd"
      },
      "line2": {
        "value": "3523 Market Rd"
      },
      "line3": {
        "value": "478 Lake Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "90600"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8694@example.com"
      },
      "phone_number": {
        "value": "7797343780"
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
date: Fri, 13 Mar 2026 06:47:47 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_f5722a99117a4799b923743f1d998fea",
  "connectorTransactionId": "0DD74975M2378641X",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2396",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:47 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f2134531e27ba",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f2134531e27ba-50a75da5ada9c0e5-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880087-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384465.525651,VS0,VE2784"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29582"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"0DD74975M2378641X\",\"intent\":\"CAPTURE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Liam Taylor\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"CREDIT\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mti_f5722a99117a4799b923743f1d998fea\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"tax_total\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mti_f5722a99117a4799b923743f1d998fea\",\"invoice_id\":\"mti_f5722a99117a4799b923743f1d998fea\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mti_f5722a99117a4799b923743f1d998fea\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"shipping\":{\"name\":{\"full_name\":\"Mia\"},\"address\":{\"address_line_1\":\"9231 Main Rd\",\"admin_area_2\":\"San Francisco\",\"postal_code\":\"72663\",\"country_code\":\"US\"}},\"payments\":{\"captures\":[{\"id\":\"32F37632HA8891932\",\"status\":\"COMPLETED\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mti_f5722a99117a4799b923743f1d998fea\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/32F37632HA8891932\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/32F37632HA8891932/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/0DD74975M2378641X\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:47:46Z\",\"update_time\":\"2026-03-13T06:47:46Z\",\"network_transaction_reference\":{\"id\":\"168483650344452\",\"network\":\"VISA\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"}}]}}],\"create_time\":\"2026-03-13T06:47:46Z\",\"update_time\":\"2026-03-13T06:47:46Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/0DD74975M2378641X\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Request-Id\":\"mti_f5722a99117a4799b923743f1d998fea\"},\"body\":{\"intent\":\"CAPTURE\",\"purchase_units\":[{\"reference_id\":\"mti_f5722a99117a4799b923743f1d998fea\",\"invoice_id\":\"mti_f5722a99117a4799b923743f1d998fea\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":{\"address_line_1\":\"9231 Main Rd\",\"postal_code\":\"72663\",\"country_code\":\"US\",\"admin_area_2\":\"San Francisco\"},\"name\":{\"full_name\":\"Mia\"}},\"items\":[{\"name\":\"Payment for invoice mti_f5722a99117a4799b923743f1d998fea\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"9821 Sunset Rd\",\"postal_code\":\"90600\",\"country_code\":\"US\",\"admin_area_2\":\"Chicago\"},\"expiry\":\"2030-08\",\"name\":\"Liam Taylor\",\"number\":\"4111111111111111\",\"security_code\":\"999\",\"attributes\":{\"vault\":null,\"verification\":null}}}}}"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"32F37632HA8891932\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
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
  -H "x-request-id: refund_refund_partial_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_partial_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_196dbb3bc1dc4f7c9efdae06c49c2787",
  "connector_transaction_id": "0DD74975M2378641X",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29582"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"32F37632HA8891932\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: refund_refund_partial_amount_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: refund_refund_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:47:49 GMT
x-request-id: refund_refund_partial_amount_req

Response contents:
{
  "connectorRefundId": "38V95108L34108004",
  "status": "REFUND_SUCCESS",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "710",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:47:49 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f86422313e3b0",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f86422313e3b0-32caa886f4f1ad56-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830083-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384468.779497,VS0,VE1401"
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"38V95108L34108004\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"30.00\"},\"seller_payable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"30.00\"},\"paypal_fee\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"30.00\"},\"total_refunded_amount\":{\"currency_code\":\"USD\",\"value\":\"30.00\"}},\"invoice_id\":\"mti_f5722a99117a4799b923743f1d998fea\",\"status\":\"COMPLETED\",\"create_time\":\"2026-03-12T23:47:48-07:00\",\"update_time\":\"2026-03-12T23:47:48-07:00\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/refunds/38V95108L34108004\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/32F37632HA8891932\",\"rel\":\"up\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/payments/captures/32F37632HA8891932/refund\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"PayPal-Request-Id\":\"mri_196dbb3bc1dc4f7c9efdae06c49c2787\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"Content-Type\":\"application/json\",\"Prefer\":\"return=representation\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":{\"amount\":{\"currency_code\":\"USD\",\"value\":\"30.00\"}}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
