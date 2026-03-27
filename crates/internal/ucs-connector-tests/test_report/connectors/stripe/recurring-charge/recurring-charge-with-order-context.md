# Connector `stripe` / Suite `recurring_charge` / Scenario `recurring_charge_with_order_context`

- Service: `RecurringPaymentService/Charge`
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
  "merchant_customer_id": "mcui_7d37c526c05f44f1bce0182758fb3907",
  "customer_name": "Noah Taylor",
  "email": {
    "value": "casey.5782@sandbox.example.com"
  },
  "phone_number": "+14236056151",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5070 Pine St"
      },
      "line2": {
        "value": "2346 Pine Dr"
      },
      "line3": {
        "value": "102 Pine Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "25022"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.7954@sandbox.example.com"
      },
      "phone_number": {
        "value": "1430011275"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2968 Sunset St"
      },
      "line2": {
        "value": "2818 Oak Rd"
      },
      "line3": {
        "value": "8834 Lake Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "76887"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2885@testmail.io"
      },
      "phone_number": {
        "value": "2461665666"
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
date: Fri, 13 Mar 2026 06:49:31 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "cus_U8gvgEPEhABOmz",
  "connectorCustomerId": "cus_U8gvgEPEhABOmz",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-methods": "GET, HEAD, PUT, PATCH, POST, DELETE",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Request-Id, Stripe-Manage-Version, Stripe-Should-Retry, X-Stripe-External-Auth-Required, X-Stripe-Privileged-Session-Required",
    "access-control-max-age": "300",
    "cache-control": "no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "679",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:31 GMT",
    "idempotency-key": "23ce0332-fe11-4278-aed8-ea86cefa2126",
    "original-request": "req_eZzrTulJQgPtRj",
    "request-id": "req_eZzrTulJQgPtRj",
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
<summary>2. setup_recurring(setup_recurring) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: stripe" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_951f3463cf37471296742f8a6d28f0d9",
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
        "value": "Mia Taylor"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Noah Wilson",
    "email": {
      "value": "alex.4297@example.com"
    },
    "id": "cust_ce0e12c4aa51410d8c3f4611d48fd010",
    "phone_number": "+912414021753",
    "connector_customer_id": "cus_U8gvgEPEhABOmz"
  },
  "setup_future_usage": "OFF_SESSION",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2968 Sunset St"
      },
      "line2": {
        "value": "2818 Oak Rd"
      },
      "line3": {
        "value": "8834 Lake Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "76887"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2885@testmail.io"
      },
      "phone_number": {
        "value": "2461665666"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  }
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
x-connector: stripe
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:33 GMT
x-request-id: setup_recurring_setup_recurring_req

Response contents:
{
  "connectorRecurringPaymentId": "seti_1TAPXMD5R7gDAGff3STHqyG8",
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
    "content-length": "1997",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:33 GMT",
    "idempotency-key": "0634cb5a-3dba-40a1-9a06-f3dc08e49094",
    "original-request": "req_qHKRfVzcC1AnI6",
    "request-id": "req_qHKRfVzcC1AnI6",
    "server": "nginx",
    "strict-transport-security": "max-age=63072000; includeSubDomains; preload",
    "stripe-should-retry": "false",
    "stripe-version": "2022-11-15",
    "vary": "Origin",
    "x-stripe-priority-routing-enabled": "true",
    "x-stripe-routing-context-priority-tier": "api-testmode",
    "x-wc": "ABGHIJ"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPXLD5R7gDAGffsDkPRIuF",
      "paymentMethodId": "pm_1TAPXLD5R7gDAGffsDkPRIuF"
    }
  },
  "merchantRecurringPaymentId": "seti_1TAPXMD5R7gDAGff3STHqyG8",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjpudWxsLCJhZGRyZXNzX3Bvc3RhbF9jb2RlX2NoZWNrIjpudWxsLCJjdmNfY2hlY2siOiJwYXNzIn0="
      }
    }
  },
  "capturedAmount": "0",
  "state": {
    "connectorCustomerId": "cus_U8gvgEPEhABOmz"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/setup_intents\",\"method\":\"POST\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"stripe-version\":\"2022-11-15\",\"Content-Type\":\"application/x-www-form-urlencoded\",\"via\":\"HyperSwitch\"},\"body\":\"confirm=true\u0026usage=off_session\u0026customer=cus_U8gvgEPEhABOmz\u0026payment_method_data%5Btype%5D=card\u0026payment_method_data%5Bcard%5D%5Bnumber%5D=4111111111111111\u0026payment_method_data%5Bcard%5D%5Bexp_month%5D=08\u0026payment_method_data%5Bcard%5D%5Bexp_year%5D=30\u0026payment_method_data%5Bcard%5D%5Bcvc%5D=999\u0026payment_method_options%5Bcard%5D%5Brequest_three_d_secure%5D=automatic\u0026metadata%5Border_id%5D=mrpi_951f3463cf37471296742f8a6d28f0d9\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_attempt\"}"
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
  -H "x-request-id: recurring_charge_recurring_charge_with_order_context_req" \
  -H "x-connector-request-reference-id: recurring_charge_recurring_charge_with_order_context_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -d @ localhost:8000 types.RecurringPaymentService/Charge <<'JSON'
{
  "merchant_charge_id": "mchi_1241915b7b274e1e92df1cbc0f204ce6",
  "connector_recurring_payment_id": {
    "connector_mandate_id": {
      "connector_mandate_id": "pm_1TAPXLD5R7gDAGffsDkPRIuF"
    }
  },
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_463199",
  "webhook_url": "https://example.com/payment/webhook",
  "return_url": "https://example.com/payment/return",
  "description": "Recurring charge with order context",
  "off_session": true,
  "test_mode": true,
  "connector_customer_id": "cus_U8gvgEPEhABOmz",
  "customer": {
    "connector_customer_id": "cus_U8gvgEPEhABOmz"
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
x-connector: stripe
x-connector-request-reference-id: recurring_charge_recurring_charge_with_order_context_ref
x-merchant-id: test_merchant
x-request-id: recurring_charge_recurring_charge_with_order_context_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:49:37 GMT
x-request-id: recurring_charge_recurring_charge_with_order_context_req

Response contents:
{
  "connectorTransactionId": "pi_3TAPXQD5R7gDAGff1uVPnCjz",
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
    "content-length": "4835",
    "content-security-policy": "base-uri 'none'; default-src 'none'; form-action 'none'; frame-ancestors 'none'; img-src 'self'; script-src 'self' 'report-sample'; style-src 'self'; worker-src 'none'; upgrade-insecure-requests; report-uri https://q.stripe.com/csp-violation?q=Fs0bD9c8eAKaAMbkesta93NT_ClKvj9FJMA2A6r9tzizbcCkwTwqCj74mLGvFFmdY2ltFH3H3sLcA-9h",
    "content-type": "application/json",
    "date": "Fri, 13 Mar 2026 06:49:37 GMT",
    "idempotency-key": "36969f77-ec6a-4130-81fa-4730d1325479",
    "original-request": "req_2W3nWjQCCfQQyQ",
    "request-id": "req_2W3nWjQCCfQQyQ",
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
  "merchantChargeId": "pi_3TAPXQD5R7gDAGff1uVPnCjz",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "pm_1TAPXLD5R7gDAGffsDkPRIuF",
      "paymentMethodId": "pm_1TAPXLD5R7gDAGffsDkPRIuF"
    }
  },
  "state": {
    "connectorCustomerId": "cus_U8gvgEPEhABOmz"
  },
  "rawConnectorResponse": {
    "value": "{\n  \"id\": \"pi_3TAPXQD5R7gDAGff1uVPnCjz\",\n  \"object\": \"payment_intent\",\n  \"amount\": 6000,\n  \"amount_capturable\": 0,\n  \"amount_details\": {\n    \"tip\": {}\n  },\n  \"amount_received\": 6000,\n  \"application\": null,\n  \"application_fee_amount\": null,\n  \"automatic_payment_methods\": null,\n  \"canceled_at\": null,\n  \"cancellation_reason\": null,\n  \"capture_method\": \"automatic\",\n  \"client_secret\": \"pi_3TAPXQD5R7gDAGff1uVPnCjz_secret_4yTxSRWdzc0ApHcofrq7ExQGc\",\n  \"confirmation_method\": \"automatic\",\n  \"created\": 1773384576,\n  \"currency\": \"usd\",\n  \"customer\": \"cus_U8gvgEPEhABOmz\",\n  \"customer_account\": null,\n  \"description\": \"Recurring charge with order context\",\n  \"excluded_payment_method_types\": null,\n  \"invoice\": null,\n  \"last_payment_error\": null,\n  \"latest_charge\": {\n    \"id\": \"ch_3TAPXQD5R7gDAGff1vGFgYsK\",\n    \"object\": \"charge\",\n    \"amount\": 6000,\n    \"amount_captured\": 6000,\n    \"amount_refunded\": 0,\n    \"amount_updates\": [],\n    \"application\": null,\n    \"application_fee\": null,\n    \"application_fee_amount\": null,\n    \"balance_transaction\": \"txn_3TAPXQD5R7gDAGff1dJqIrky\",\n    \"billing_details\": {\n      \"address\": {\n        \"city\": null,\n        \"country\": null,\n        \"line1\": null,\n        \"line2\": null,\n        \"postal_code\": null,\n        \"state\": null\n      },\n      \"email\": null,\n      \"name\": null,\n      \"phone\": null,\n      \"tax_id\": null\n    },\n    \"calculated_statement_descriptor\": \"BERNARD\",\n    \"captured\": true,\n    \"created\": 1773384576,\n    \"currency\": \"usd\",\n    \"customer\": \"cus_U8gvgEPEhABOmz\",\n    \"description\": \"Recurring charge with order context\",\n    \"destination\": null,\n    \"dispute\": null,\n    \"disputed\": false,\n    \"failure_balance_transaction\": null,\n    \"failure_code\": null,\n    \"failure_message\": null,\n    \"fraud_details\": {},\n    \"invoice\": null,\n    \"livemode\": false,\n    \"metadata\": {\n      \"order_id\": \"mchi_1241915b7b274e1e92df1cbc0f204ce6\"\n    },\n    \"on_behalf_of\": null,\n    \"order\": null,\n    \"outcome\": {\n      \"advice_code\": null,\n      \"network_advice_code\": null,\n      \"network_decline_code\": null,\n      \"network_status\": \"approved_by_network\",\n      \"reason\": null,\n      \"risk_level\": \"normal\",\n      \"risk_score\": 23,\n      \"seller_message\": \"Payment complete.\",\n      \"type\": \"authorized\"\n    },\n    \"paid\": true,\n    \"payment_intent\": \"pi_3TAPXQD5R7gDAGff1uVPnCjz\",\n    \"payment_method\": \"pm_1TAPXLD5R7gDAGffsDkPRIuF\",\n    \"payment_method_details\": {\n      \"card\": {\n        \"amount_authorized\": 6000,\n        \"authorization_code\": \"481760\",\n        \"brand\": \"visa\",\n        \"checks\": {\n          \"address_line1_check\": null,\n          \"address_postal_code_check\": null,\n          \"cvc_check\": \"pass\"\n        },\n        \"country\": \"US\",\n        \"exp_month\": 8,\n        \"exp_year\": 2030,\n        \"extended_authorization\": {\n          \"status\": \"disabled\"\n        },\n        \"fingerprint\": \"aEed1rfhfa5JNpoz\",\n        \"funding\": \"credit\",\n        \"incremental_authorization\": {\n          \"status\": \"unavailable\"\n        },\n        \"installments\": null,\n        \"last4\": \"1111\",\n        \"mandate\": null,\n        \"moto\": null,\n        \"multicapture\": {\n          \"status\": \"unavailable\"\n        },\n        \"network\": \"visa\",\n        \"network_token\": {\n          \"used\": false\n        },\n        \"network_transaction_id\": \"976910110049114\",\n        \"overcapture\": {\n          \"maximum_amount_capturable\": 6000,\n          \"status\": \"unavailable\"\n        },\n        \"regulated_status\": \"unregulated\",\n        \"three_d_secure\": null,\n        \"wallet\": null\n      },\n      \"type\": \"card\"\n    },\n    \"radar_options\": {},\n    \"receipt_email\": null,\n    \"receipt_number\": null,\n    \"receipt_url\": \"https://pay.stripe.com/receipts/payment/CAcaFwoVYWNjdF8xTTdmVGFENVI3Z0RBR2ZmKIHnzs0GMgbUkrlyRuA6LBZTDHDvTGsxDqsAZu0vbZTF-ZUYeEkSSszgna5BgUR2s2993AxG_PIDxkTw\",\n    \"refunded\": false,\n    \"review\": null,\n    \"shipping\": null,\n    \"source\": null,\n    \"source_transfer\": null,\n    \"statement_descriptor\": null,\n    \"statement_descriptor_suffix\": null,\n    \"status\": \"succeeded\",\n    \"transfer_data\": null,\n    \"transfer_group\": null\n  },\n  \"livemode\": false,\n  \"metadata\": {\n    \"order_id\": \"mchi_1241915b7b274e1e92df1cbc0f204ce6\"\n  },\n  \"next_action\": null,\n  \"on_behalf_of\": null,\n  \"payment_method\": \"pm_1TAPXLD5R7gDAGffsDkPRIuF\",\n  \"payment_method_configuration_details\": null,\n  \"payment_method_options\": {\n    \"card\": {\n      \"installments\": null,\n      \"mandate_options\": null,\n      \"network\": null,\n      \"request_three_d_secure\": \"automatic\"\n    }\n  },\n  \"payment_method_types\": [\n    \"card\"\n  ],\n  \"processing\": null,\n  \"receipt_email\": null,\n  \"review\": null,\n  \"setup_future_usage\": null,\n  \"shipping\": null,\n  \"source\": null,\n  \"statement_descriptor\": null,\n  \"statement_descriptor_suffix\": null,\n  \"status\": \"succeeded\",\n  \"transfer_data\": null,\n  \"transfer_group\": null\n}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://api.stripe.com/v1/payment_intents\",\"method\":\"POST\",\"headers\":{\"Authorization\":\"Bearer ***MASKED***",\"Content-Type\":\"application/x-www-form-urlencoded\",\"stripe-version\":\"2022-11-15\",\"via\":\"HyperSwitch\"},\"body\":\"amount=6000\u0026currency=USD\u0026metadata%5Border_id%5D=mchi_1241915b7b274e1e92df1cbc0f204ce6\u0026return_url=https%3A%2F%2Fexample.com%2Fpayment%2Freturn\u0026confirm=true\u0026payment_method=pm_1TAPXLD5R7gDAGffsDkPRIuF\u0026customer=cus_U8gvgEPEhABOmz\u0026description=Recurring+charge+with+order+context\u0026capture_method=automatic\u0026off_session=true\u0026payment_method_types%5B0%5D=card\u0026expand%5B0%5D=latest_charge\"}"
  },
  "capturedAmount": "6000",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhZGRyZXNzX2xpbmUxX2NoZWNrIjpudWxsLCJhZGRyZXNzX3Bvc3RhbF9jb2RlX2NoZWNrIjpudWxsLCJjdmNfY2hlY2siOiJwYXNzIn0="
      }
    },
    "extendedAuthorizationResponseData": ***MASKED***
      "extendedAuthenticationApplied": false
    },
    "isOvercaptureEnabled": false
  },
  "incrementalAuthorizationAllowed": ***MASKED***
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../recurring-charge.md) | [Back to Overview](../../../test_overview.md)
