# Connector `stripe` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
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
  "merchant_customer_id": "mcui_d0d87430129549f18056eff1e86b27c4",
  "customer_name": "Mia Taylor",
  "email": {
    "value": "casey.7870@testmail.io"
  },
  "phone_number": "+15881368371",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9901 Oak Ave"
      },
      "line2": {
        "value": "6752 Lake Rd"
      },
      "line3": {
        "value": "9259 Sunset Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53724"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4574@example.com"
      },
      "phone_number": {
        "value": "3389151267"
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
        "value": "7424 Main Rd"
      },
      "line2": {
        "value": "7168 Sunset Ave"
      },
      "line3": {
        "value": "1640 Pine Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "26613"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.4754@testmail.io"
      },
      "phone_number": {
        "value": "9981047440"
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
date: Fri, 13 Mar 2026 06:49:13 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8guMeZntb1CtA",
  "connectorCustomerId": "cus_U8guMeZntb1CtA",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "670",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:12 GMT",
    "idempotency-key": "792564f9-714c-4f7e-9439-45e396b494b3",
    "original-request": "req_44kfuA0JEwAjbm",
    "request-id": "req_44kfuA0JEwAjbm",
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_178b832078794f68bc57bb590b3c14ea",
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
        "value": "Mia Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "morgan.4062@sandbox.example.com"
    },
    "id": "cust_a7e3e2760db04a3da5df88276c8587d5",
    "phone_number": "+445984508631",
    "connector_customer_id": "cus_U8guMeZntb1CtA"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9901 Oak Ave"
      },
      "line2": {
        "value": "6752 Lake Rd"
      },
      "line3": {
        "value": "9259 Sunset Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53724"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4574@example.com"
      },
      "phone_number": {
        "value": "3389151267"
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
        "value": "7424 Main Rd"
      },
      "line2": {
        "value": "7168 Sunset Ave"
      },
      "line3": {
        "value": "1640 Pine Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "26613"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.4754@testmail.io"
      },
      "phone_number": {
        "value": "9981047440"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:14 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TAPX3D5R7gDAGff0cdZ2KCm",
  "connectorTransactionId": "pi_3TAPX3D5R7gDAGff0cdZ2KCm",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "5540",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:14 GMT",
    "idempotency-key": "5c1cf3a8-bec6-4c75-9801-c1e95fae9b01",
    "original-request": "req_8KeZpzGA599ZMp",
    "request-id": "req_8KeZpzGA599ZMp",
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
    "connectorCustomerId": "cus_U8guMeZntb1CtA"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPX3D5R7gDAGff0cdZ2KCm\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 0,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 6000,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"automatic\",\n  \"client_secret\": \"pi_3TAPX3D5R7gDAGff0cdZ2KCm_secret_vhwt0j3OXbBdt6BEl4SBmSBdA\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384553,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8guMeZntb1CtA\",\n  \"customer_account\": null,\n  \"description\": \"No3DS auto capture card payment (credit)\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPX3D5R7gDAGff09JFIk7A\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 6000,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": \"txn_3TAPX3D5R7gDAGff0PbdVGDM\",\n    \"billing_details\": {\n      \"address\": {\n        \"city\": \"Austin\",\n        \"country\": \"US\",\n        \"line1\": \"7424 Main Rd\",\n        \"line2\": \"7168 Sunset Ave\",\n        \"postal_code\": \"26613\",\n        \"state\": \"CA\"\n      },\n      \"email\": \"morgan.4754@testmail.io\",\n      \"name\": \"Mia Taylor\",\n      \"phone\": \"9981047440\",\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": true,\n    \"created\": 1773384553,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8guMeZntb1CtA\",\n    \"description\": \"No3DS auto capture card payment (credit)\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mti_178b832078794f68bc57bb590b3c14ea\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 13,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPX3D5R7gDAGff0cdZ2KCm\",\n    \"payment_method\": \"pm_1TAPX3D5R7gDAGff8chYt5yz\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"864005\",\n        \"brand\": \"visa\",\n        \"checks\": {\n          \"address_line1_check\": \"pass\",\n          \"address_postal_code_check\": \"pass\",\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKOrmzs0GMgY_B0VWOUw6LBYx2fxmQLESmKgeTKg9Gw-ZMwkidSi6ymGbAtN2uG3ggxht-bC56UW2VrlL\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": {\n      \"address\": {\n        \"city\": \"San Francisco\",\n        \"country\": \"US\",\n        \"line1\": \"9901 Oak Ave\",\n        \"line2\": \"6752 Lake Rd\",\n        \"postal_code\": \"53724\",\n        \"state\": \"CA\"\n      },\n      \"carrier\": null,\n      \"name\": \"Mia Wilson\",\n      \"phone\": \"+913389151267\",\n      \"tracking_number\": null\n    },\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mti_178b832078794f68bc57bb590b3c14ea\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPX3D5R7gDAGff8chYt5yz\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": \"on_session\",\n  \"shipping\": {\n    \"address\": {\n      \"city\": \"San Francisco\",\n      \"country\": \"US\",\n      \"line1\": \"9901 Oak Ave\",\n      \"line2\": \"6752 Lake Rd\",\n      \"postal_code\": \"53724\",\n      \"state\": \"CA\"\n    },\n    \"carrier\": null,\n    \"name\": \"Mia Wilson\",\n    \"phone\": \"+913389151267\",\n    \"tracking_number\": null\n  },\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"succeeded\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"method\":\"POST\",\"headers\":{\"stripe-version\":\"2022-11-15\",\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/x-www-form-urlencoded\"},\"body\":\"amount=6000\u0026currency=USD\u0026metadata%5Border_id%5D=mti_178b832078794f68bc57bb590b3c14ea\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026confirm=true\u0026customer=cus_U8guMeZntb1CtA\u0026description=No3DS+auto+capture+card+payment+%28credit%29\u0026shipping%5Baddress%5D%5Bcity%5D=San+Francisco\u0026shipping%5Baddress%5D%5Bcountry%5D=US\u0026shipping%5Baddress%5D%5Bline1%5D=9901+Oak+Ave\u0026shipping%5Baddress%5D%5Bline2%5D=6752+Lake+Rd\u0026shipping%5Baddress%5D%5Bpostal_code%5D=53724\u0026shipping%5Baddress%5D%5Bstate%5D=CA\u0026shipping%5Bname%5D=Mia+Wilson\u0026shipping%5Bphone%5D=%2B913389151267\u0026payment_method_data%5Bbilling_details%5D%5Bemail%5D=morgan.4754%40testmail.io\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcountry%5D=US\u0026payment_method_data%5Bbilling_details%5D%5Bname%5D=Mia+Taylor\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcity%5D=Austin\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline1%5D=7424+Main+Rd\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline2%5D=7168+Sunset+Ave\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bpostal_code%5D=26613\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bstate%5D=CA\u0026payment_method_data%5Bbilling_details%5D%5Bphone%5D=9981047440\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026capture_method=automatic\u0026setup_future_usage=on_session\u0026off_session=false\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_charge\"}"
  },
  "capturedAmount": "6000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPX3D5R7gDAGff8chYt5yz",
      "paymentMethodId": "pm_1TAPX3D5R7gDAGff8chYt5yz"
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
  -H "x-request-id: get_sync_payment_req" \
  -H "x-connector-request-reference-id: get_sync_payment_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "pi_3TAPX3D5R7gDAGff0cdZ2KCm",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "cus_U8guMeZntb1CtA"
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
x-connector: stripe
x-connector-request-reference-id: get_sync_payment_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:15 GMT
x-request-id: get_sync_payment_req

Response contents:
{
  "connectorTransactionId": "pi_3TAPX3D5R7gDAGff0cdZ2KCm",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "5540",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:15 GMT",
    "request-id": "req_GIZgfA8hkVCkRU",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPX3D5R7gDAGff8chYt5yz",
      "paymentMethodId": "pm_1TAPX3D5R7gDAGff8chYt5yz"
    }
  },
  "networkTransactionId": "976910110049114",
  "capturedAmount": "6000",
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
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPX3D5R7gDAGff0cdZ2KCm\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 0,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 6000,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"automatic\",\n  \"client_secret\": \"pi_3TAPX3D5R7gDAGff0cdZ2KCm_secret_vhwt0j3OXbBdt6BEl4SBmSBdA\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384553,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8guMeZntb1CtA\",\n  \"customer_account\": null,\n  \"description\": \"No3DS auto capture card payment (credit)\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPX3D5R7gDAGff09JFIk7A\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 6000,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": \"txn_3TAPX3D5R7gDAGff0PbdVGDM\",\n    \"billing_details\": {\n      \"address\": {\n        \"city\": \"Austin\",\n        \"country\": \"US\",\n        \"line1\": \"7424 Main Rd\",\n        \"line2\": \"7168 Sunset Ave\",\n        \"postal_code\": \"26613\",\n        \"state\": \"CA\"\n      },\n      \"email\": \"morgan.4754@testmail.io\",\n      \"name\": \"Mia Taylor\",\n      \"phone\": \"9981047440\",\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": true,\n    \"created\": 1773384553,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8guMeZntb1CtA\",\n    \"description\": \"No3DS auto capture card payment (credit)\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mti_178b832078794f68bc57bb590b3c14ea\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 13,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPX3D5R7gDAGff0cdZ2KCm\",\n    \"payment_method\": \"pm_1TAPX3D5R7gDAGff8chYt5yz\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"864005\",\n        \"brand\": \"visa\",\n        \"checks\": {\n          \"address_line1_check\": \"pass\",\n          \"address_postal_code_check\": \"pass\",\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKOrmzs0GMgYFy1GU-H06LBabXa7QCZ1lezp_Na6ORV7tKw1kcaCWGFeFpgQw4gYNAYZBWex849fdKLYd\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": {\n      \"address\": {\n        \"city\": \"San Francisco\",\n        \"country\": \"US\",\n        \"line1\": \"9901 Oak Ave\",\n        \"line2\": \"6752 Lake Rd\",\n        \"postal_code\": \"53724\",\n        \"state\": \"CA\"\n      },\n      \"carrier\": null,\n      \"name\": \"Mia Wilson\",\n      \"phone\": \"+913389151267\",\n      \"tracking_number\": null\n    },\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mti_178b832078794f68bc57bb590b3c14ea\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPX3D5R7gDAGff8chYt5yz\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": \"on_session\",\n  \"shipping\": {\n    \"address\": {\n      \"city\": \"San Francisco\",\n      \"country\": \"US\",\n      \"line1\": \"9901 Oak Ave\",\n      \"line2\": \"6752 Lake Rd\",\n      \"postal_code\": \"53724\",\n      \"state\": \"CA\"\n    },\n    \"carrier\": null,\n    \"name\": \"Mia Wilson\",\n    \"phone\": \"+913389151267\",\n    \"tracking_number\": null\n  },\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"succeeded\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents/pi_3TAPX3D5R7gDAGff0cdZ2KCm?expand[0]=latest_charge\",\"method\":\"GET\",\"headers\":{\"Content-Type\":\"application/x-www-form-urlencoded\",\"via\":\"HyperSwitch\",\"stripe-version\":\"2022-11-15\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
