# Connector `rapyd` / Suite `PaymentService/Void` / Scenario `Void | Authorized Payment`

- Service: `PaymentService/Void`
- Scenario Key: `void_authorized_payment`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
```

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
  "merchant_transaction_id": "mti_18bb941068c74cd98add6fa3",
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
    "name": "Liam Miller",
    "email": {
      "value": "casey.2013@sandbox.example.com"
    },
    "id": "cust_a32af45053754ad78bee84c1",
    "phone_number": "+11676149929",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6539 Sunset Ln"
      },
      "line2": {
        "value": "7911 Pine St"
      },
      "line3": {
        "value": "4973 Main Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41125"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3514@example.com"
      },
      "phone_number": {
        "value": "7911807627"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9520 Pine Rd"
      },
      "line2": {
        "value": "1345 Main Rd"
      },
      "line3": {
        "value": "675 Sunset Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "16998"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.4910@sandbox.example.com"
      },
      "phone_number": {
        "value": "6097666287"
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
date: Mon, 13 Apr 2026 16:25:43 GMT
x-request-id: PaymentService/Authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_18bb941068c74cd98add6fa3",
  "connectorTransactionId": "payment_57b78a6c0dc1b5c7859f43a861ab42b5",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd40cba3ab630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:43 GMT",
    "etag": "W/\"8bd-yA3kK+I8+SuW7hGIgvFVPc0onXQ\"",
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
  -H "x-request-id: PaymentService/Void_void_authorized_payment_req" \
  -H "x-connector-request-reference-id: PaymentService/Void_void_authorized_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "payment_57b78a6c0dc1b5c7859f43a861ab42b5",
  "merchant_void_id": "mvi_0e123d1847dd40028195ef07",
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
  "cancellation_reason": "requested_by_customer"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Cancel an authorized payment that has not been captured. Releases held funds
// back to the customer's payment method when a transaction cannot be completed.
rpc Void ( .types.PaymentServiceVoidRequest ) returns ( .types.PaymentServiceVoidResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: PaymentService/Void_void_authorized_payment_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Void_void_authorized_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:44 GMT
x-request-id: PaymentService/Void_void_authorized_payment_req

Response contents:
{
  "connectorTransactionId": "payment_57b78a6c0dc1b5c7859f43a861ab42b5",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd412daefb630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:44 GMT",
    "etag": "W/\"8bd-+Jz8wxPSxFn5uxxLQrjMLWtT5EA\"",
    "server": "cloudflare",
    "set-cookie": ***MASKED***"
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "merchantVoidId": "mti_18bb941068c74cd98add6fa3",
  "rawConnectorRequest": "***MASKED***"


Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentservice-void.md) | [Back to Overview](../../../test_overview.md)
