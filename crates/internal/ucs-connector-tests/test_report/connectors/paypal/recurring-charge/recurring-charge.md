# Connector `paypal` / Suite `recurring_charge` / Scenario `recurring_charge`

- Service: `RecurringPaymentService/Charge`
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
date: Fri, 13 Mar 2026 06:48:21 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
  },
  "expiresInSeconds": "29545",
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
<summary>2. setup_recurring(setup_recurring) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: paypal" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_06a8066828784d73a5fb404b730efc7e",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
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
        "value": "Emma Brown"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ava Wilson",
    "email": {
      "value": "casey.9435@example.com"
    },
    "id": "cust_adbf116e6d1744a881e7b489e3d3cf2b",
    "phone_number": "+441614860719"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29545"
    }
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "713 Sunset Rd"
      },
      "line2": {
        "value": "2366 Oak Dr"
      },
      "line3": {
        "value": "7533 Pine Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35270"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5947@example.com"
      },
      "phone_number": {
        "value": "6811940101"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:22 GMT
x-request-id: setup_recurring_setup_recurring_req

Response contents:
{
  "connectorRecurringPaymentId": "5cd55852r3099093l",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "582",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:22 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f78168375e965",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f78168375e965-ec6abcd68e16583a-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsss1830092-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384501.467883,VS0,VE994"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "5cd55852r3099093l"
    }
  },
  "merchantRecurringPaymentId": "5cd55852r3099093l",
  "capturedAmount": "0",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29545"
    }
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v3/vault/payment-tokens/\",\"method\":\"POST\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"PayPal-Request-Id\":\"mrpi_06a8066828784d73a5fb404b730efc7e\",\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\",\"Prefer\":\"return=representation\"},\"body\":{\"payment_source\":{\"card\":{\"billing_address\":{\"address_line_1\":\"713 Sunset Rd\",\"postal_code\":\"35270\",\"country_code\":\"US\",\"admin_area_2\":\"San Francisco\"},\"expiry\":\"2030-08\",\"name\":\"Mia Brown\",\"number\":\"4111111111111111\"}}}}"
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
  -H "x-request-id: recurring_charge_recurring_charge_req" \
  -H "x-connector-request-reference-id: recurring_charge_recurring_charge_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.RecurringPaymentService/Charge <<'JSON'
{
  "merchant_charge_id": "mchi_fffae565d56e4a9abab095c6d6a6ea84",
  "connector_recurring_payment_id": {
    "connector_mandate_id": {
      "connector_mandate_id": "5cd55852r3099093l"
    }
  },
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method_type": "CREDIT",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expires_in_seconds": "29545"
    }
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Charge using an existing stored recurring payment instruction. Processes repeat payments for
// subscriptions or recurring billing without collecting payment details.
rpc Charge ( .types.RecurringPaymentServiceChargeRequest ) returns ( .types.RecurringPaymentServiceChargeResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: paypal
x-connector-request-reference-id: recurring_charge_recurring_charge_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: recurring_charge_recurring_charge_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:25 GMT
x-request-id: recurring_charge_recurring_charge_req

Response contents:
{
  "connectorTransactionId": "9T5133939S920105S",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2242",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:25 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f975661d3c18b",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f975661d3c18b-6e8e09a57be45b12-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-cache": "MISS, MISS",
    "x-cache-hits": "0, 0",
    "x-served-by": "cache-sin-wsat1880084-SIN, cache-bom-vanm7210067-BOM",
    "x-timer": "S1773384503.681243,VS0,VE2792"
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"53A39720FM979040T\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  },
  "merchantChargeId": "mchi_fffae565d56e4a9abab095c6d6a6ea84",
  "mandateReference": {
    "connectorMandateId": {}
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AALDBy1KjC6R64l4BnblKtors4dsNxLjgcjLr9oxRW5p9qxkmnymJdOy1OXk9z6Bl3y28P6cFAgytM59ffYllZVtZpmNoQ"
      },
      "expiresInSeconds": "29545"
    }
  },
  "rawConnectorResponse": {
    "value": "{\"id\":\"9T5133939S920105S\",\"intent\":\"CAPTURE\",\"status\":\"COMPLETED\",\"payment_source\":{\"card\":{\"name\":\"Mia Brown\",\"last_digits\":\"1111\",\"expiry\":\"2030-08\",\"brand\":\"VISA\",\"type\":\"CREDIT\",\"bin_details\":{}}},\"purchase_units\":[{\"reference_id\":\"mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"handling\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"tax_total\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"insurance\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"shipping_discount\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"payee\":{\"email_address\":\"sb-itwmi27136406@business.example.com\",\"merchant_id\":\"DUM69V9DDNYEJ\"},\"description\":\"Payment for invoice mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"invoice_id\":\"mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"soft_descriptor\":\"TEST STORE\",\"items\":[{\"name\":\"Payment for invoice mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":{\"currency_code\":\"USD\",\"value\":\"0.00\"},\"quantity\":\"1\"}],\"payments\":{\"captures\":[{\"id\":\"53A39720FM979040T\",\"status\":\"COMPLETED\",\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"final_capture\":true,\"seller_protection\":{\"status\":\"NOT_ELIGIBLE\"},\"seller_receivable_breakdown\":{\"gross_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"net_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"}},\"invoice_id\":\"mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/53A39720FM979040T\",\"rel\":\"self\",\"method\":\"GET\"},{\"href\":\"https://api.sandbox.paypal.com/v2/payments/captures/53A39720FM979040T/refund\",\"rel\":\"refund\",\"method\":\"POST\"},{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/9T5133939S920105S\",\"rel\":\"up\",\"method\":\"GET\"}],\"create_time\":\"2026-03-13T06:48:25Z\",\"update_time\":\"2026-03-13T06:48:25Z\",\"network_transaction_reference\":{\"id\":\"080401486574539\",\"network\":\"VISA\"},\"processor_response\":{\"avs_code\":\"A\",\"cvv_code\":\"M\",\"response_code\":\"0000\"}}]}}],\"create_time\":\"2026-03-13T06:48:25Z\",\"update_time\":\"2026-03-13T06:48:25Z\",\"links\":[{\"href\":\"https://api.sandbox.paypal.com/v2/checkout/orders/9T5133939S920105S\",\"rel\":\"self\",\"method\":\"GET\"}]}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api-m.sandbox.paypal.com/v2/checkout/orders\",\"method\":\"POST\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"via\":\"HyperSwitch\",\"Prefer\":\"return=representation\",\"PayPal-Partner-Attribution-Id\":\"HyperSwitchlegacy_Ecom\",\"PayPal-Request-Id\":\"mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"Content-Type\":\"application/json\"},\"body\":{\"intent\":\"CAPTURE\",\"purchase_units\":[{\"reference_id\":\"mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"invoice_id\":\"mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"custom_id\":null,\"amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\",\"breakdown\":{\"item_total\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax_total\":null,\"shipping\":{\"currency_code\":\"USD\",\"value\":\"0.00\"}}},\"shipping\":{\"address\":null,\"name\":{\"full_name\":null}},\"items\":[{\"name\":\"Payment for invoice mchi_fffae565d56e4a9abab095c6d6a6ea84\",\"quantity\":1,\"unit_amount\":{\"currency_code\":\"USD\",\"value\":\"60.00\"},\"tax\":null}]}],\"payment_source\":{\"card\":{\"vault_id\":\"5cd55852r3099093l\"}}}}"
  },
  "incrementalAuthorizationAllowed": ***MASKED***
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../recurring-charge.md) | [Back to Overview](../../../test_overview.md)
