# Connector `stripe` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
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
  "merchant_customer_id": "mcui_79c9b33db2d740cfa015214501ac0aec",
  "customer_name": "Noah Brown",
  "email": {
    "value": "riley.1952@example.com"
  },
  "phone_number": "+911904658516",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "728 Oak St"
      },
      "line2": {
        "value": "3098 Main Ln"
      },
      "line3": {
        "value": "2031 Market Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53987"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3789@example.com"
      },
      "phone_number": {
        "value": "1205934552"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3740 Lake Ln"
      },
      "line2": {
        "value": "5338 Pine Rd"
      },
      "line3": {
        "value": "8044 Oak Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39674"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2868@testmail.io"
      },
      "phone_number": {
        "value": "9991554323"
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
date: Fri, 13 Mar 2026 06:49:18 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8guS3jLNsVMTX",
  "connectorCustomerId": "cus_U8guS3jLNsVMTX",
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
    "date": "Fri, 13 Mar 2026 06:49:18 GMT",
    "idempotency-key": "6d47426b-48c5-4b27-bb63-f88ae13c3004",
    "original-request": "req_x6dcon5qyw9grb",
    "request-id": "req_x6dcon5qyw9grb",
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
  "merchant_transaction_id": "mti_82e4cadd60bf43248ad271d8d402f6d7",
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
        "value": "Ethan Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "sam.7456@sandbox.example.com"
    },
    "id": "cust_cf33728ac1384fa0944ea25b0a10f4c0",
    "phone_number": "+917326707607",
    "connector_customer_id": "cus_U8guS3jLNsVMTX"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "728 Oak St"
      },
      "line2": {
        "value": "3098 Main Ln"
      },
      "line3": {
        "value": "2031 Market Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53987"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3789@example.com"
      },
      "phone_number": {
        "value": "1205934552"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3740 Lake Ln"
      },
      "line2": {
        "value": "5338 Pine Rd"
      },
      "line3": {
        "value": "8044 Oak Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39674"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2868@testmail.io"
      },
      "phone_number": {
        "value": "9991554323"
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
date: Fri, 13 Mar 2026 06:49:20 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TAPX9D5R7gDAGff0xFiegwL",
  "connectorTransactionId": "pi_3TAPX9D5R7gDAGff0xFiegwL",
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
    "content-length": "5524",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:20 GMT",
    "idempotency-key": "037725ed-4b19-413e-8ecd-45bf44bddd78",
    "original-request": "req_2n6W1Asdb1b2OC",
    "request-id": "req_2n6W1Asdb1b2OC",
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
    "connectorCustomerId": "cus_U8guS3jLNsVMTX"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPX9D5R7gDAGff0xFiegwL\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 0,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 6000,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"automatic\",\n  \"client_secret\": \"pi_3TAPX9D5R7gDAGff0xFiegwL_secret_eQydxRShJBkhOW2CrmjBhmKrW\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384559,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8guS3jLNsVMTX\",\n  \"customer_account\": null,\n  \"description\": \"No3DS auto capture card payment (credit)\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPX9D5R7gDAGff0sYQGJP0\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 6000,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": \"txn_3TAPX9D5R7gDAGff0l6EwMOS\",\n    \"billing_details\": {\n      \"address\": {\n        \"city\": \"Los Angeles\",\n        \"country\": \"US\",\n        \"line1\": \"3740 Lake Ln\",\n        \"line2\": \"5338 Pine Rd\",\n        \"postal_code\": \"39674\",\n        \"state\": \"CA\"\n      },\n      \"email\": \"sam.2868@testmail.io\",\n      \"name\": \"Ethan Smith\",\n      \"phone\": \"9991554323\",\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": true,\n    \"created\": 1773384559,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8guS3jLNsVMTX\",\n    \"description\": \"No3DS auto capture card payment (credit)\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mti_82e4cadd60bf43248ad271d8d402f6d7\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 40,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPX9D5R7gDAGff0xFiegwL\",\n    \"payment_method\": \"pm_1TAPX9D5R7gDAGffXlcoeDkp\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"461919\",\n        \"brand\": \"visa\",\n        \"checks\": {\n          \"address_line1_check\": \"pass\",\n          \"address_postal_code_check\": \"pass\",\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKPDmzs0GMgbtCoa_QtA6LBbvrVxqRjoJmGMqeWXAuDbhIvrvolQVGXHjHSpdX5rYU5HEde_RXXdsu1-o\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": {\n      \"address\": {\n        \"city\": \"Austin\",\n        \"country\": \"US\",\n        \"line1\": \"728 Oak St\",\n        \"line2\": \"3098 Main Ln\",\n        \"postal_code\": \"53987\",\n        \"state\": \"CA\"\n      },\n      \"carrier\": null,\n      \"name\": \"Noah Taylor\",\n      \"phone\": \"+911205934552\",\n      \"tracking_number\": null\n    },\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mti_82e4cadd60bf43248ad271d8d402f6d7\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPX9D5R7gDAGffXlcoeDkp\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": \"on_session\",\n  \"shipping\": {\n    \"address\": {\n      \"city\": \"Austin\",\n      \"country\": \"US\",\n      \"line1\": \"728 Oak St\",\n      \"line2\": \"3098 Main Ln\",\n      \"postal_code\": \"53987\",\n      \"state\": \"CA\"\n    },\n    \"carrier\": null,\n    \"name\": \"Noah Taylor\",\n    \"phone\": \"+911205934552\",\n    \"tracking_number\": null\n  },\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"succeeded\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/x-www-form-urlencoded\",\"via\":\"HyperSwitch\",\"stripe-version\":\"2022-11-15\",\"Authorization\":\"Bearer ***MASKED***"},\"body\":\"amount=6000\u0026currency=USD\u0026metadata%5Border_id%5D=mti_82e4cadd60bf43248ad271d8d402f6d7\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026confirm=true\u0026customer=cus_U8guS3jLNsVMTX\u0026description=No3DS+auto+capture+card+payment+%28credit%29\u0026shipping%5Baddress%5D%5Bcity%5D=Austin\u0026shipping%5Baddress%5D%5Bcountry%5D=US\u0026shipping%5Baddress%5D%5Bline1%5D=728+Oak+St\u0026shipping%5Baddress%5D%5Bline2%5D=3098+Main+Ln\u0026shipping%5Baddress%5D%5Bpostal_code%5D=53987\u0026shipping%5Baddress%5D%5Bstate%5D=CA\u0026shipping%5Bname%5D=Noah+Taylor\u0026shipping%5Bphone%5D=%2B911205934552\u0026payment_method_data%5Bbilling_details%5D%5Bemail%5D=sam.2868%40testmail.io\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcountry%5D=US\u0026payment_method_data%5Bbilling_details%5D%5Bname%5D=Ethan+Smith\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcity%5D=Los+Angeles\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline1%5D=3740+Lake+Ln\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline2%5D=5338+Pine+Rd\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bpostal_code%5D=39674\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bstate%5D=CA\u0026payment_method_data%5Bbilling_details%5D%5Bphone%5D=9991554323\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026capture_method=automatic\u0026setup_future_usage=on_session\u0026off_session=false\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_charge\"}"
  },
  "capturedAmount": "6000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPX9D5R7gDAGffXlcoeDkp",
      "paymentMethodId": "pm_1TAPX9D5R7gDAGffXlcoeDkp"
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
<summary>3. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_bcbd955609164d85b4ce6fc9a9c786a9",
  "connector_transaction_id": "pi_3TAPX9D5R7gDAGff0xFiegwL",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "cus_U8guS3jLNsVMTX"
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
x-connector: stripe
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:21 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "re_3TAPX9D5R7gDAGff03FZQBjE",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "714",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:21 GMT",
    "idempotency-key": "731aa242-549a-4ce3-9a69-8c697ccd2cfc",
    "original-request": "req_EwSbAY5Cm5R3QE",
    "request-id": "req_EwSbAY5Cm5R3QE",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"re_3TAPX9D5R7gDAGff03FZQBjE\",\n  \"object\": \"refund\",\n  \"amount\": 6000,\n  \"balance_transaction\": \"txn_3TAPX9D5R7gDAGff0vrJQlyV\",\n  \"charge\": \"ch_3TAPX9D5R7gDAGff0sYQGJP0\",\n  \"created\": 1773384560,\n  \"currency\": \"usd\",\n  \"destination_details\": {\n    \"card\": {\n      \"reference_status\": \"pending\",\n      \"reference_type\": \"acquirer_reference_number\",\n      \"type\": \"refund\"\n    },\n    \"type\": \"card\"\n  },\n  \"metadata\": {\n    \"is_refund_id_as_reference\": \"true\",\n    \"order_id\": \"mri_bcbd955609164d85b4ce6fc9a9c786a9\"\n  },\n  \"payment_intent\": \"pi_3TAPX9D5R7gDAGff0xFiegwL\",\n  \"reason\": null,\n  \"receipt_number\": null,\n  \"source_transfer_reversal\": null,\n  \"status\": \"succeeded\",\n  \"transfer_reversal\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/refunds\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"stripe-version\":\"2022-11-15\",\"Content-Type\":\"application/x-www-form-urlencoded\"},\"body\":\"amount=6000\u0026payment_intent=pi_3TAPX9D5R7gDAGff0xFiegwL\u0026metadata%5Border_id%5D=mri_bcbd955609164d85b4ce6fc9a9c786a9\u0026metadata%5Bis_refund_id_as_reference%5D=true\"}"
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "pi_3TAPX9D5R7gDAGff0xFiegwL",
  "refund_id": "re_3TAPX9D5R7gDAGff03FZQBjE",
  "state": {
    "connector_customer_id": "cus_U8guS3jLNsVMTX"
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
x-connector: stripe
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:22 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "merchantRefundId": "re_3TAPX9D5R7gDAGff03FZQBjE",
  "connectorRefundId": "re_3TAPX9D5R7gDAGff03FZQBjE",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "754",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:22 GMT",
    "request-id": "req_Gj3QC7QsYQkMy6",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"re_3TAPX9D5R7gDAGff03FZQBjE\",\n  \"object\": \"refund\",\n  \"amount\": 6000,\n  \"balance_transaction\": \"txn_3TAPX9D5R7gDAGff0vrJQlyV\",\n  \"charge\": \"ch_3TAPX9D5R7gDAGff0sYQGJP0\",\n  \"created\": 1773384560,\n  \"currency\": \"usd\",\n  \"destination_details\": {\n    \"card\": {\n      \"reference\": \"505340253892383\",\n      \"reference_status\": \"available\",\n      \"reference_type\": \"acquirer_reference_number\",\n      \"type\": \"refund\"\n    },\n    \"type\": \"card\"\n  },\n  \"metadata\": {\n    \"is_refund_id_as_reference\": \"true\",\n    \"order_id\": \"mri_bcbd955609164d85b4ce6fc9a9c786a9\"\n  },\n  \"payment_intent\": \"pi_3TAPX9D5R7gDAGff0xFiegwL\",\n  \"reason\": null,\n  \"receipt_number\": null,\n  \"source_transfer_reversal\": null,\n  \"status\": \"succeeded\",\n  \"transfer_reversal\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/refunds/re_3TAPX9D5R7gDAGff03FZQBjE\",\"method\":\"GET\",\"headers\":{\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"stripe-version\":\"2022-11-15\",\"Content-Type\":\"application/x-www-form-urlencoded\"},\"body\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
