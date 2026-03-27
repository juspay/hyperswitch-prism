# Connector `stripe` / Suite `capture` / Scenario `capture_full_amount`

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
  "merchant_customer_id": "mcui_6e127cf7e7b9449a81ec7859c6aae88c",
  "customer_name": "Ethan Wilson",
  "email": {
    "value": "jordan.5031@testmail.io"
  },
  "phone_number": "+14341568288",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "7974 Oak Ln"
      },
      "line2": {
        "value": "3881 Lake Ave"
      },
      "line3": {
        "value": "9537 Market Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "25949"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8787@sandbox.example.com"
      },
      "phone_number": {
        "value": "3074108123"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "9524 Sunset Blvd"
      },
      "line2": {
        "value": "50 Oak Blvd"
      },
      "line3": {
        "value": "712 Market St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85294"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5616@example.com"
      },
      "phone_number": {
        "value": "7636197023"
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
date: Fri, 13 Mar 2026 06:48:40 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8guHBEhVUw2ML",
  "connectorCustomerId": "cus_U8guHBEhVUw2ML",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "673",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:40 GMT",
    "idempotency-key": "1c6cc539-e8e8-4905-93cf-16864275738b",
    "original-request": "req_XqcI9JTSmqOeba",
    "request-id": "req_XqcI9JTSmqOeba",
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
  "merchant_transaction_id": "mti_82ec2933d9b74a19a3c712551d2d2ad0",
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
        "value": "Mia Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "sam.3993@example.com"
    },
    "id": "cust_66a53959d4f9400393268de10ec72059",
    "phone_number": "+914215973402",
    "connector_customer_id": "cus_U8guHBEhVUw2ML"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "7974 Oak Ln"
      },
      "line2": {
        "value": "3881 Lake Ave"
      },
      "line3": {
        "value": "9537 Market Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "25949"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8787@sandbox.example.com"
      },
      "phone_number": {
        "value": "3074108123"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "9524 Sunset Blvd"
      },
      "line2": {
        "value": "50 Oak Blvd"
      },
      "line3": {
        "value": "712 Market St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85294"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5616@example.com"
      },
      "phone_number": {
        "value": "7636197023"
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
date: Fri, 13 Mar 2026 06:48:42 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "pi_3TAPWXD5R7gDAGff0PxM5OyP",
  "connectorTransactionId": "pi_3TAPWXD5R7gDAGff0PxM5OyP",
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
    "content-length": "5560",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:42 GMT",
    "idempotency-key": "c539a8be-600f-4ff8-a333-ba1cb6e9a269",
    "original-request": "req_MPqWPr1vexjL1u",
    "request-id": "req_MPqWPr1vexjL1u",
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
    "connectorCustomerId": "cus_U8guHBEhVUw2ML"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPWXD5R7gDAGff0PxM5OyP\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 6000,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 0,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"manual\",\n  \"client_secret\": \"pi_3TAPWXD5R7gDAGff0PxM5OyP_secret_kLn4aXGIVoPXx2Sr4wP17eQlZ\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384521,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8guHBEhVUw2ML\",\n  \"customer_account\": null,\n  \"description\": \"No3DS manual capture card payment (credit)\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPWXD5R7gDAGff0HgzfVBi\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 0,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": null,\n    \"billing_details\": {\n      \"address\": {\n        \"city\": \"San Francisco\",\n        \"country\": \"US\",\n        \"line1\": \"9524 Sunset Blvd\",\n        \"line2\": \"50 Oak Blvd\",\n        \"postal_code\": \"85294\",\n        \"state\": \"CA\"\n      },\n      \"email\": \"riley.5616@example.com\",\n      \"name\": \"Liam Brown\",\n      \"phone\": \"7636197023\",\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": false,\n    \"created\": 1773384521,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8guHBEhVUw2ML\",\n    \"description\": \"No3DS manual capture card payment (credit)\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mti_82ec2933d9b74a19a3c712551d2d2ad0\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 55,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPWXD5R7gDAGff0PxM5OyP\",\n    \"payment_method\": \"pm_1TAPWXD5R7gDAGffAT6pqtCR\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"005047\",\n        \"brand\": \"visa\",\n        \"capture_before\": 1773989321,\n        \"checks\": {\n          \"address_line1_check\": \"pass\",\n          \"address_postal_code_check\": \"pass\",\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKMrmzs0GMgZeGJ2RZ2E6LBYLpZFCYbGLpa6iQx23Qs7bkvy90XMA7ElJBzk0dCXIwgMsjO9QG_E-A7Gw\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": {\n      \"address\": {\n        \"city\": \"Los Angeles\",\n        \"country\": \"US\",\n        \"line1\": \"7974 Oak Ln\",\n        \"line2\": \"3881 Lake Ave\",\n        \"postal_code\": \"25949\",\n        \"state\": \"CA\"\n      },\n      \"carrier\": null,\n      \"name\": \"Emma Brown\",\n      \"phone\": \"+913074108123\",\n      \"tracking_number\": null\n    },\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mti_82ec2933d9b74a19a3c712551d2d2ad0\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPWXD5R7gDAGffAT6pqtCR\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": \"on_session\",\n  \"shipping\": {\n    \"address\": {\n      \"city\": \"Los Angeles\",\n      \"country\": \"US\",\n      \"line1\": \"7974 Oak Ln\",\n      \"line2\": \"3881 Lake Ave\",\n      \"postal_code\": \"25949\",\n      \"state\": \"CA\"\n    },\n    \"carrier\": null,\n    \"name\": \"Emma Brown\",\n    \"phone\": \"+913074108123\",\n    \"tracking_number\": null\n  },\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"requires_capture\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/x-www-form-urlencoded\",\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"stripe-version\":\"2022-11-15\"},\"body\":\"amount=6000\u0026currency=USD\u0026metadata%5Border_id%5D=mti_82ec2933d9b74a19a3c712551d2d2ad0\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026confirm=true\u0026customer=cus_U8guHBEhVUw2ML\u0026description=No3DS+manual+capture+card+payment+%28credit%29\u0026shipping%5Baddress%5D%5Bcity%5D=Los+Angeles\u0026shipping%5Baddress%5D%5Bcountry%5D=US\u0026shipping%5Baddress%5D%5Bline1%5D=7974+Oak+Ln\u0026shipping%5Baddress%5D%5Bline2%5D=3881+Lake+Ave\u0026shipping%5Baddress%5D%5Bpostal_code%5D=25949\u0026shipping%5Baddress%5D%5Bstate%5D=CA\u0026shipping%5Bname%5D=Emma+Brown\u0026shipping%5Bphone%5D=%2B913074108123\u0026payment_method_data%5Bbilling_details%5D%5Bemail%5D=riley.5616%40example.com\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcountry%5D=US\u0026payment_method_data%5Bbilling_details%5D%5Bname%5D=Liam+Brown\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bcity%5D=San+Francisco\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline1%5D=9524+Sunset+Blvd\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bline2%5D=50+Oak+Blvd\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bpostal_code%5D=85294\u0026payment_method_data%5Bbilling_details%5D%5Baddress%5D%5Bstate%5D=CA\u0026payment_method_data%5Bbilling_details%5D%5Bphone%5D=7636197023\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026capture_method=manual\u0026setup_future_usage=on_session\u0026off_session=false\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_charge\"}"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPWXD5R7gDAGffAT6pqtCR",
      "paymentMethodId": "pm_1TAPWXD5R7gDAGffAT6pqtCR"
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
  -H "x-request-id: capture_capture_full_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_full_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "pi_3TAPWXD5R7gDAGff0PxM5OyP",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_4ef9bf69fb3a4aa6a7537dd63d9a75cc",
  "state": {
    "connector_customer_id": "cus_U8guHBEhVUw2ML"
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
x-connector-request-reference-id: capture_capture_full_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:48:43 GMT
x-request-id: capture_capture_full_amount_req

Response contents:
{
  "connectorTransactionId": "pi_3TAPWXD5R7gDAGff0PxM5OyP",
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
    "content-length": "1845",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:48:43 GMT",
    "idempotency-key": "c34b46b6-c75a-4b8c-8461-713e51f4e5ab",
    "original-request": "req_sV2iBn8UEOc3H5",
    "request-id": "req_sV2iBn8UEOc3H5",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "merchantCaptureId": "pi_3TAPWXD5R7gDAGff0PxM5OyP",
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents/pi_3TAPWXD5R7gDAGff0PxM5OyP/capture\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/x-www-form-urlencoded\",\"stripe-version\":\"2022-11-15\"},\"body\":\"amount_to_capture=6000\"}"
  },
  "capturedAmount": "6000",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPWXD5R7gDAGffAT6pqtCR",
      "paymentMethodId": "pm_1TAPWXD5R7gDAGffAT6pqtCR"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
