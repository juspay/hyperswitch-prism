# Connector `worldpayxml` / Suite `void` / Scenario `void_without_cancellation_reason`

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
  "merchant_transaction_id": "mti_59ea712c2d7843b7a8f6cb1d",
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
        "value": "Liam Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Wilson",
    "email": {
      "value": "jordan.2832@example.com"
    },
    "id": "cust_f169aa7189c04c7aa9d2f773",
    "phone_number": "+914050919815"
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
        "value": "Miller"
      },
      "line1": {
        "value": "4170 Main Ln"
      },
      "line2": {
        "value": "4126 Sunset St"
      },
      "line3": {
        "value": "1231 Main Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64422"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3507@sandbox.example.com"
      },
      "phone_number": {
        "value": "8170113725"
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
        "value": "8043 Main Ave"
      },
      "line2": {
        "value": "8417 Pine St"
      },
      "line3": {
        "value": "1587 Market St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83335"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.2516@sandbox.example.com"
      },
      "phone_number": {
        "value": "5695142780"
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
date: Tue, 24 Mar 2026 07:11:30 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_59ea712c2d7843b7a8f6cb1d",
  "connectorTransactionId": "mti_59ea712c2d7843b7a8f6cb1d",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-security-policy-report-only": "default-src https: data: 'unsafe-eval' 'unsafe-inline'; object-src 'none'; report-uri https://secure.worldpay.com/public/CspReport",
    "content-type": "text/plain",
    "date": "Tue, 24 Mar 2026 07:11:30 GMT",
    "expires": "Tue, 24 Mar 2026 07:11:30 GMT",
    "p3p": "CP=\"NON\"",
    "pragma": "no-cache",
    "set-cookie": "machine=0a854017;Secure;path=/",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "vary": "Accept-Encoding",
    "x-cnection": "close",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "211490",
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
  "connector_transaction_id": "mti_59ea712c2d7843b7a8f6cb1d",
  "merchant_void_id": "mvi_95228b36b5694aebb00c6b12",
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
date: Tue, 24 Mar 2026 07:11:31 GMT
x-request-id: void_void_without_cancellation_reason_req

Response contents:
{
  "connectorTransactionId": "mti_59ea712c2d7843b7a8f6cb1d",
  "status": "VOID_INITIATED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-length": "359",
    "content-security-policy-report-only": "default-src https: data: 'unsafe-eval' 'unsafe-inline'; object-src 'none'; report-uri https://secure.worldpay.com/public/CspReport",
    "content-type": "text/plain",
    "date": "Tue, 24 Mar 2026 07:11:31 GMT",
    "expires": "Tue, 24 Mar 2026 07:11:31 GMT",
    "p3p": "CP=\"NON\"",
    "pragma": "no-cache",
    "set-cookie": "machine=0a854016;Secure;path=/",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "x-cnection": "close",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
  },
  "merchantVoidId": "mti_59ea712c2d7843b7a8f6cb1d",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
