# Connector `worldpayvantiv` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "VOID_INITIATED"
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_14767930f56c4368874da892",
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
        "value": "Emma Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Smith",
    "email": {
      "value": "jordan.5885@example.com"
    },
    "id": "cust_03396a51a9204c198d15f108",
    "phone_number": "+441012406576"
  },
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
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3217 Main Dr"
      },
      "line2": {
        "value": "1948 Sunset Rd"
      },
      "line3": {
        "value": "8494 Pine Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "77100"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2164@sandbox.example.com"
      },
      "phone_number": {
        "value": "1961831384"
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
        "value": "4193 Market Rd"
      },
      "line2": {
        "value": "7500 Sunset Ave"
      },
      "line3": {
        "value": "9851 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71078"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6926@sandbox.example.com"
      },
      "phone_number": {
        "value": "9592104247"
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
  "locale": "en-US"
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:18:03 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_14767930f56c4368874da892",
  "connectorTransactionId": "84086940574875819",
  "status": "AUTHORIZING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-type": "text/xml;charset=ISO-8859-1",
    "date": "Tue, 24 Mar 2026 07:18:03 GMT",
    "expires": "Tue, 24 Mar 2026 07:18:03 GMT",
    "pragma": "no-cache",
    "set-cookie": "JSESSIONID=535C06D8E649C05220F1EB5E97F52DC4; Path=/vap; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "vary": "Accept-Encoding"
  },
  "networkTransactionId": "565063725812885",
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
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
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: void_void_without_cancellation_reason_req" \
  -H "x-connector-request-reference-id: void_void_without_cancellation_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "84086940574875819",
  "merchant_void_id": "mvi_9aeb882ee5564dd6a534c390",
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
// Cancel an authorized payment before capture. Releases held funds back to
// customer, typically used when orders are cancelled or abandoned.
rpc Void ( .types.PaymentServiceVoidRequest ) returns ( .types.PaymentServiceVoidResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: void_void_without_cancellation_reason_ref
x-merchant-id: test_merchant
x-request-id: void_void_without_cancellation_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:18:04 GMT
x-request-id: void_void_without_cancellation_reason_req

Response contents:
{
  "connectorTransactionId": "84086940574875843",
  "status": "VOID_INITIATED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "503",
    "content-type": "text/xml;charset=ISO-8859-1",
    "date": "Tue, 24 Mar 2026 07:18:04 GMT",
    "expires": "Tue, 24 Mar 2026 07:18:04 GMT",
    "pragma": "no-cache",
    "set-cookie": "JSESSIONID=EF90E9C887010BE4CEBD9BB14686E195; Path=/vap; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains"
  },
  "merchantVoidId": "void_mvi_9aeb882ee5564dd6a534c390",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
