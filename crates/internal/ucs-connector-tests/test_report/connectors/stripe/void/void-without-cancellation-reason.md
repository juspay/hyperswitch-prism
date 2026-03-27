# Connector `stripe` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_e5e0f4e171174a0e81c6b293bc1ed299",
  "customer_name": "Emma Miller",
  "email": {
    "value": "morgan.6774@example.com"
  },
  "phone_number": "+915384540770",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8458 Oak Ln"
      },
      "line2": {
        "value": "3228 Lake Rd"
      },
      "line3": {
        "value": "821 Sunset Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60525"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1479@sandbox.example.com"
      },
      "phone_number": {
        "value": "6151260010"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8886 Oak Dr"
      },
      "line2": {
        "value": "4043 Main Rd"
      },
      "line3": {
        "value": "9314 Oak Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "84486"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1953@testmail.io"
      },
      "phone_number": {
        "value": "9416896020"
      },
      "phone_country_code": "+91"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: stripe
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:57 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8guACGYqGchnI",
  "connectorCustomerId": "cus_U8guACGYqGchnI",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "672",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:57 GMT",
    "idempotency-key": "8a4d8854-827a-41e2-9679-cbf6211606b3",
    "original-request": "req_SzqaOpPJbWB04F",
    "request-id": "req_SzqaOpPJbWB04F",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  }
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
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_6e3fafdc8d3d43629b8f1c6743f5d0a2",
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
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "jordan.3400@example.com"
    },
    "id": "cust_d19cebb6827046d981eca0001fd6e37c",
    "phone_number": "+13299455814",
    "connector_customer_id": "cus_U8guACGYqGchnI"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8458 Oak Ln"
      },
      "line2": {
        "value": "3228 Lake Rd"
      },
      "line3": {
        "value": "821 Sunset Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60525"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1479@sandbox.example.com"
      },
      "phone_number": {
        "value": "6151260010"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8886 Oak Dr"
      },
      "line2": {
        "value": "4043 Main Rd"
      },
      "line3": {
        "value": "9314 Oak Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "84486"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1953@testmail.io"
      },
      "phone_number": {
        "value": "9416896020"
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
  "test_mode": true
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
x-connector: stripe
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:59 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TAPWoD5R7gDAGff16Vox5y4",
  "connectorTransactionId": "pi_3TAPWoD5R7gDAGff16Vox5y4",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "5543",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:59 GMT",
    "idempotency-key": "e662a7d1-a826-4cc4-b90b-e560292b4806",
    "original-request": "req_cpDwF8E1dzHe9P",
    "request-id": "req_cpDwF8E1dzHe9P",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "networkTransactionId": "976910110049114",
  "state": {
    "connectorCustomerId": "cus_U8guACGYqGchnI"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPWoD5R7gDAGff16Vox5y4\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 6000,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 0,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"manual\",\n  \"client_secret\": \"pi_3TAPWoD5R7gDAGff16Vox5y4_secret_gBEmAokQSIpLRbYMA2YJwxw4j\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384538,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8guACGYqGchnI\",\n  \"customer_account\": null,\n  \"description\": \"No3DS manual capture card payment (credit)\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPWoD5R7gDAGff1ioWLWww\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 0,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": null,\n    \"billing_details\": {\n      \"address\": {\n        \"city\": \"Los Angeles\",\n        \"country\": \"US\",\n        \"line1\": \"8886 Oak Dr\",\n        \"line2\": \"4043 Main Rd\",\n        \"postal_code\": \"84486\",\n        \"state\": \"CA\"\n      },\n      \"email\": \"alex.1953@testmail.io\",\n      \"name\": \"Mia Wilson\",\n      \"phone\": \"9416896020\",\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": false,\n    \"created\": 1773384538,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8guACGYqGchnI\",\n    \"description\": \"No3DS manual capture card payment (credit)\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mti_6e3fafdc8d3d43629b8f1c6743f5d0a2\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 13,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPWoD5R7gDAGff16Vox5y4\",\n    \"payment_method\": \"pm_1TAPWoD5R7gDAGffBfqdXRBV\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"978452\",\n        \"brand\": \"visa\",\n        \"capture_before\": 1773989338,\n        \"checks\": {\n          \"address_line1_check\": \"pass\",\n          \"address_postal_code_check\": \"pass\",\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKNrmzs0GMgb_TQdU7I06LBa03Ekip_ipWChYE6Bp5RmZxuxOPpFJdJpapOGzE9mTLqvMmpsqVzzL-A51\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": {\n      \"address\": {\n        \"city\": \"Austin\",\n        \"country\": \"US\",\n        \"line1\": \"8458 Oak Ln\",\n        \"line2\": \"3228 Lake Rd\",\n        \"postal_code\": \"60525\",\n        \"state\": \"CA\"\n      },\n      \"carrier\": null,\n      \"name\": \"Ethan Smith\",\n      \"phone\": \"+916151260010\",\n      \"tracking_number\": null\n    },\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mti_6e3fafdc8d3d43629b8f1c6743f5d0a2\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPWoD5R7gDAGffBfqdXRBV\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": \"on_session\",\n  \"shipping\": {\n    \"address\": {\n      \"city\": \"Austin\",\n      \"country\": \"US\",\n      \"line1\": \"8458 Oak Ln\",\n      \"line2\": \"3228 Lake Rd\",\n      \"postal_code\": \"60525\",\n      \"state\": \"CA\"\n    },\n    \"carrier\": null,\n    \"name\": \"Ethan Smith\",\n    \"phone\": \"+916151260010\",\n    \"tracking_number\": null\n  },\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"requires_capture\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/x-www-form-urlencoded\",\"stripe-version\":\"2022-11-15\"},\"body\":\"amount=6000\u0026currency=USD\u0026metadata%5Border_id%5D=mti_6e3fafdc8d3d43629b8f1c6743f5d0a2\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026confirm=true\u0026customer=cus_U8guACGYqGchnI\u0026description=No3DS+manual+capture+card+payment+%28credit%29\u0026shipping%5Baddress%5D%5Bcity%5D=Austin\u0026shipping%5Baddress%5D%5Bcountry%5D=US\u0026shipping%5Baddress%5D%5Bline1%5D=8458+Oak+Ln\u0026shipping%5Baddress%5D%5Bline2%5D=3228+Lake+Rd\u0026shipping%5Baddress%5D%5Bpostal_code%5D=60525\u0026shipping%5Baddress%5D%5Bstate%5D=CA\u0026shipping%5Bname%5D=Ethan+Smith\u0026shipping%5Bphone%5D=%2B916151260010\u0026payment_method_data%5Bbilling_details%5D%5Bemail%5D=alex.1953%40testmail.io\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcountry%5D=US\u0026payment_method_data%5Bbilling_details%5D%5Bname%5D=Mia+Wilson\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcity%5D=Los+Angeles\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline1%5D=8886+Oak+Dr\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline2%5D=4043+Main+Rd\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bpostal_code%5D=84486\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bstate%5D=CA\u0026payment_method_data%5Bbilling_details%5D%5Bphone%5D=9416896020\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026capture_method=manual\u0026setup_future_usage=on_session\u0026off_session=false\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_charge\"}"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPWoD5R7gDAGffBfqdXRBV",
      "paymentMethodId": "pm_1TAPWoD5R7gDAGffBfqdXRBV"
    }
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjoicGFzcyIsImFkZHJlc3NfcG9zdGFsX2NvZGVfY2hlY2siOiJwYXNzIiwiY3ZjX2NoZWNrIjoicGFzcyJ9"
      }
    },
    "extendedAuthorizationResponseData": ***MASKED***
      "extendedAuthenticationApplied": false
    },
    "isOvercaptureEnabled": false
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
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: void_void_without_cancellation_reason_req" \
  -H "x-connector-request-reference-id: void_void_without_cancellation_reason_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "pi_3TAPWoD5R7gDAGff16Vox5y4",
  "merchant_void_id": "mvi_48ba4291dc7744f2bd75ff52d1e97cf8",
  "state": {
    "connector_customer_id": "cus_U8guACGYqGchnI"
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
x-connector: stripe
x-connector-request-reference-id: void_void_without_cancellation_reason_ref
x-merchant-id: test_merchant
x-request-id: void_void_without_cancellation_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:00 GMT
x-request-id: void_void_without_cancellation_reason_req

Response contents:
{
  "connectorTransactionId": "pi_3TAPWoD5R7gDAGff16Vox5y4",
  "status": "VOIDED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "1842",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:00 GMT",
    "idempotency-key": "c20647da-9c8b-4765-a879-c396079e2fa9",
    "original-request": "req_nZ8WJIqKXX55pP",
    "request-id": "req_nZ8WJIqKXX55pP",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "merchantVoidId": "pi_3TAPWoD5R7gDAGff16Vox5y4",
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents/pi_3TAPWoD5R7gDAGff16Vox5y4/cancel\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/x-www-form-urlencoded\",\"stripe-version\":\"2022-11-15\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":\"\"}"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPWoD5R7gDAGffBfqdXRBV",
      "paymentMethodId": "pm_1TAPWoD5R7gDAGffBfqdXRBV"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
