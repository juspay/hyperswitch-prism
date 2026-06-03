# Connector `rapyd` / Suite `PaymentService/Capture` / Scenario `Capture | Partial Amount`

- Service: `PaymentService/Capture`
- Scenario Key: `capture_partial_amount`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. PaymentService/Authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_a1c00812e6014dd7b3e3c88d",
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
        "value": "Noah Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Miller",
    "email": {
      "value": "riley.3856@sandbox.example.com"
    },
    "id": "cust_47c6c3fde7104dc0b17b3f68",
    "phone_number": "+919199089680",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5586 Sunset Blvd"
      },
      "line2": {
        "value": "8217 Market Blvd"
      },
      "line3": {
        "value": "8154 Pine Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59320"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.7182@sandbox.example.com"
      },
      "phone_number": {
        "value": "3157587479"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5367 Market Ln"
      },
      "line2": {
        "value": "6993 Market St"
      },
      "line3": {
        "value": "9225 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "26826"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6182@example.com"
      },
      "phone_number": {
        "value": "8920999280"
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:24 GMT
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_a1c00812e6014dd7b3e3c88d",
  "connectorTransactionId": "payment_13bb228052d5e03e25d77fa6c297027e",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd391db20b630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:24 GMT",
    "etag": "W/\"8bc-SJnYc+8GaRgjCldUjTcocJmhl/g\"",
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
  -H "x-request-id: PaymentService/Capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: PaymentService/Capture_capture_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "payment_13bb228052d5e03e25d77fa6c297027e",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_725584f731334dc084298cfe",
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
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment by transferring funds. Captures the authorized
// amount to complete the transaction and move funds to your merchant account.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Capture_capture_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:25 GMT
x-request-id: PaymentService/Capture_capture_partial_amount_req

Response contents:
{
  "connectorTransactionId": "payment_13bb228052d5e03e25d77fa6c297027e",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd3996e6bb630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:25 GMT",
    "etag": "W/\"8c3-OSqW5iCo3RrFsyJgZBNFqD7GRvI\"",
    "server": "cloudflare",
    "set-cookie": ***MASKED***"
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "merchantCaptureId": "mti_a1c00812e6014dd7b3e3c88d",
  "rawConnectorRequest": "***MASKED***"


Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentservice-capture.md) | [Back to Overview](../../../test_overview.md)
