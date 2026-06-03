# Connector `rapyd` / Suite `PaymentService/Get` / Scenario `Get | Sync Payment`

- Service: `PaymentService/Get`
- Scenario Key: `sync_payment`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. PaymentService/Authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c16743dce7d0471ca254cf9c",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Taylor",
    "email": {
      "value": "sam.8159@testmail.io"
    },
    "id": "cust_720d19a62f1c453db93a9cb1",
    "phone_number": "+17442563423",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "5682 Oak Blvd"
      },
      "line2": {
        "value": "7643 Sunset Ave"
      },
      "line3": {
        "value": "2218 Pine Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71250"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2702@testmail.io"
      },
      "phone_number": {
        "value": "9796362414"
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
        "value": "1677 Sunset Dr"
      },
      "line2": {
        "value": "3159 Lake Ln"
      },
      "line3": {
        "value": "9854 Market St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "76241"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.5802@example.com"
      },
      "phone_number": {
        "value": "3931842989"
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
  "locale": "en-US",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  },
  "order_details": []
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
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:29 GMT
x-request-id: PaymentService/Authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_c16743dce7d0471ca254cf9c",
  "connectorTransactionId": "payment_34713b01aa6df6dd94939aa36bb50f78",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd3b11fa5b630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:29 GMT",
    "etag": "W/\"901-NCTe6xmn4TASYvxOSwVr5xP2BSM\"",
    "server": "cloudflare",
    "set-cookie": ***MASKED***"
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "state": {
    "connectorCustomerId": ""
  },
  "rawConnectorResponse": "***MASKED***",
  "rawConnectorRequest": "***MASKED***"


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
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Get_sync_payment_req" \
  -H "x-connector-request-reference-id: PaymentService/Get_sync_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "payment_34713b01aa6df6dd94939aa36bb50f78",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
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
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Get_sync_payment_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Get_sync_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:29 GMT
x-request-id: PaymentService/Get_sync_payment_req

Response contents:
{
  "connectorTransactionId": "payment_34713b01aa6df6dd94939aa36bb50f78",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd3b829bbb630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:29 GMT",
    "etag": "W/\"901-M0ewlpeEhVrWO/qkwuqwLpDDB3U\"",
    "server": "cloudflare",
    "set-cookie": ***MASKED***"
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "rawConnectorResponse": "***MASKED***",
  "rawConnectorRequest": "***MASKED***",
  "merchantTransactionId": "mti_c16743dce7d0471ca254cf9c"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentservice-get.md) | [Back to Overview](../../../test_overview.md)
