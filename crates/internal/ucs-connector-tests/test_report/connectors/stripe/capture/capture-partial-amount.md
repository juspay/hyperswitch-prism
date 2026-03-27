# Connector `stripe` / Suite `capture` / Scenario `capture_partial_amount`

- Service: `PaymentService/Capture`
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
  "merchant_customer_id": "mcui_3d48c18caa2f4dc39e879e361de61f7e",
  "customer_name": "Ava Wilson",
  "email": {
    "value": "riley.1588@sandbox.example.com"
  },
  "phone_number": "+914970106478",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "7548 Main Ave"
      },
      "line2": {
        "value": "4122 Market Dr"
      },
      "line3": {
        "value": "97 Market Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55027"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8392@sandbox.example.com"
      },
      "phone_number": {
        "value": "3333467894"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2233 Main Blvd"
      },
      "line2": {
        "value": "2583 Sunset Ave"
      },
      "line3": {
        "value": "7672 Main Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30180"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.6585@testmail.io"
      },
      "phone_number": {
        "value": "9739784294"
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
date: Fri, 13 Mar 2026 06:48:44 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8guKclKHuzYUz",
  "connectorCustomerId": "cus_U8guKclKHuzYUz",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "678",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:44 GMT",
    "idempotency-key": "965d6415-bf96-4386-872c-7fdf54ac6871",
    "original-request": "req_eadltQOt4LsNuZ",
    "request-id": "req_eadltQOt4LsNuZ",
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
  "merchant_transaction_id": "mti_97e97a1702d34743b04c028d7346c157",
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
        "value": "Liam Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "alex.2235@sandbox.example.com"
    },
    "id": "cust_6da7a5b354404cfda01eb4f6a724af65",
    "phone_number": "+15784202640",
    "connector_customer_id": "cus_U8guKclKHuzYUz"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "7548 Main Ave"
      },
      "line2": {
        "value": "4122 Market Dr"
      },
      "line3": {
        "value": "97 Market Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55027"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8392@sandbox.example.com"
      },
      "phone_number": {
        "value": "3333467894"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2233 Main Blvd"
      },
      "line2": {
        "value": "2583 Sunset Ave"
      },
      "line3": {
        "value": "7672 Main Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30180"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.6585@testmail.io"
      },
      "phone_number": {
        "value": "9739784294"
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
date: Fri, 13 Mar 2026 06:48:45 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TAPWaD5R7gDAGff1xbYStJD",
  "connectorTransactionId": "pi_3TAPWaD5R7gDAGff1xbYStJD",
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
    "content-length": "5564",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:45 GMT",
    "idempotency-key": "8f1b64e9-5beb-4698-b610-af60a9186f36",
    "original-request": "req_qSl1dUYgA4hlz2",
    "request-id": "req_qSl1dUYgA4hlz2",
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
    "connectorCustomerId": "cus_U8guKclKHuzYUz"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPWaD5R7gDAGff1xbYStJD\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 6000,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 0,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"manual\",\n  \"client_secret\": \"pi_3TAPWaD5R7gDAGff1xbYStJD_secret_uhW719YPDElgo2O2hThqT3RBK\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384524,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8guKclKHuzYUz\",\n  \"customer_account\": null,\n  \"description\": \"No3DS manual capture card payment (credit)\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPWaD5R7gDAGff1nimLAa2\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 0,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": null,\n    \"billing_details\": {\n      \"address\": {\n        \"city\": \"Los Angeles\",\n        \"country\": \"US\",\n        \"line1\": \"2233 Main Blvd\",\n        \"line2\": \"2583 Sunset Ave\",\n        \"postal_code\": \"30180\",\n        \"state\": \"CA\"\n      },\n      \"email\": \"casey.6585@testmail.io\",\n      \"name\": \"Ethan Wilson\",\n      \"phone\": \"9739784294\",\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": false,\n    \"created\": 1773384525,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8guKclKHuzYUz\",\n    \"description\": \"No3DS manual capture card payment (credit)\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mti_97e97a1702d34743b04c028d7346c157\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 15,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPWaD5R7gDAGff1xbYStJD\",\n    \"payment_method\": \"pm_1TAPWaD5R7gDAGffNjP8qFDy\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"459133\",\n        \"brand\": \"visa\",\n        \"capture_before\": 1773989325,\n        \"checks\": {\n          \"address_line1_check\": \"pass\",\n          \"address_postal_code_check\": \"pass\",\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKM3mzs0GMgZqP9W9Brc6LBanm-WreU_Kvfst0wFIKY901IRJjpNMe3c7xU98y4gNGHcuIfJdsI8qD_aN\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": {\n      \"address\": {\n        \"city\": \"Seattle\",\n        \"country\": \"US\",\n        \"line1\": \"7548 Main Ave\",\n        \"line2\": \"4122 Market Dr\",\n        \"postal_code\": \"55027\",\n        \"state\": \"CA\"\n      },\n      \"carrier\": null,\n      \"name\": \"Liam Johnson\",\n      \"phone\": \"+913333467894\",\n      \"tracking_number\": null\n    },\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mti_97e97a1702d34743b04c028d7346c157\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPWaD5R7gDAGffNjP8qFDy\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": \"on_session\",\n  \"shipping\": {\n    \"address\": {\n      \"city\": \"Seattle\",\n      \"country\": \"US\",\n      \"line1\": \"7548 Main Ave\",\n      \"line2\": \"4122 Market Dr\",\n      \"postal_code\": \"55027\",\n      \"state\": \"CA\"\n    },\n    \"carrier\": null,\n    \"name\": \"Liam Johnson\",\n    \"phone\": \"+913333467894\",\n    \"tracking_number\": null\n  },\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"requires_capture\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"method\":\"POST\",\"headers\":{\"stripe-version\":\"2022-11-15\",\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/x-www-form-urlencoded\"},\"body\":\"amount=6000\u0026currency=USD\u0026metadata%5Border_id%5D=mti_97e97a1702d34743b04c028d7346c157\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026confirm=true\u0026customer=cus_U8guKclKHuzYUz\u0026description=No3DS+manual+capture+card+payment+%28credit%29\u0026shipping%5Baddress%5D%5Bcity%5D=Seattle\u0026shipping%5Baddress%5D%5Bcountry%5D=US\u0026shipping%5Baddress%5D%5Bline1%5D=7548+Main+Ave\u0026shipping%5Baddress%5D%5Bline2%5D=4122+Market+Dr\u0026shipping%5Baddress%5D%5Bpostal_code%5D=55027\u0026shipping%5Baddress%5D%5Bstate%5D=CA\u0026shipping%5Bname%5D=Liam+Johnson\u0026shipping%5Bphone%5D=%2B913333467894\u0026payment_method_data%5Bbilling_details%5D%5Bemail%5D=casey.6585%40testmail.io\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcountry%5D=US\u0026payment_method_data%5Bbilling_details%5D%5Bname%5D=Ethan+Wilson\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcity%5D=Los+Angeles\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline1%5D=2233+Main+Blvd\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline2%5D=2583+Sunset+Ave\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bpostal_code%5D=30180\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bstate%5D=CA\u0026payment_method_data%5Bbilling_details%5D%5Bphone%5D=9739784294\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026capture_method=manual\u0026setup_future_usage=on_session\u0026off_session=false\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_charge\"}"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPWaD5R7gDAGffNjP8qFDy",
      "paymentMethodId": "pm_1TAPWaD5R7gDAGffNjP8qFDy"
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
  -H "x-request-id: capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_partial_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "pi_3TAPWaD5R7gDAGff1xbYStJD",
  "amount_to_capture": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_e7ffa09edb1c44e0a0bc47f32b446066",
  "state": {
    "connector_customer_id": "cus_U8guKclKHuzYUz"
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
x-connector: stripe
x-connector-request-reference-id: capture_capture_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:47 GMT
x-request-id: capture_capture_partial_amount_req

Response contents:
{
  "connectorTransactionId": "pi_3TAPWaD5R7gDAGff1xbYStJD",
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
    "content-length": "1846",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:47 GMT",
    "idempotency-key": "a742f73c-1e33-4970-8352-4f1f3a9f8232",
    "original-request": "req_XTZZN0jdyJEqMh",
    "request-id": "req_XTZZN0jdyJEqMh",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "merchantCaptureId": "pi_3TAPWaD5R7gDAGff1xbYStJD",
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents/pi_3TAPWaD5R7gDAGff1xbYStJD/capture\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/x-www-form-urlencoded\",\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"stripe-version\":\"2022-11-15\"},\"body\":\"amount_to_capture=3000\"}"
  },
  "capturedAmount": "3000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPWaD5R7gDAGffNjP8qFDy",
      "paymentMethodId": "pm_1TAPWaD5R7gDAGffNjP8qFDy"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
