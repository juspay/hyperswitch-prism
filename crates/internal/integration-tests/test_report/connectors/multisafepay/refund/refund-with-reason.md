# Connector `multisafepay` / Suite `refund` / Scenario `refund_with_reason`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'status': expected one of ["CHARGED", "AUTHORIZED"], got "AUTHENTICATION_PENDING"
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_9a7ea33c63b8426d9aec8061",
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
        "value": "Emma Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "morgan.3735@example.com"
    },
    "id": "cust_cb8df1c6364046c1bb0708d4",
    "phone_number": "+13759507748"
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
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2208 Sunset Rd"
      },
      "line2": {
        "value": "5579 Main Ln"
      },
      "line3": {
        "value": "2457 Sunset Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39811"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6334@example.com"
      },
      "phone_number": {
        "value": "5029113710"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "9414 Main Ln"
      },
      "line2": {
        "value": "7747 Lake Ave"
      },
      "line3": {
        "value": "9167 Sunset Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "43436"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.7791@example.com"
      },
      "phone_number": {
        "value": "5376382684"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:11:24 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "unknown",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, Content-Type, x-readme-api-explorer",
    "access-control-allow-methods": "OPTIONS,GET,POST,PATCH,DELETE",
    "cache-control": "no-store, no-cache, must-revalidate",
    "content-security-policy": "default-src 'none'",
    "content-type": "application/json; charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:11:12 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "pragma": "no-cache",
    "server": "Apache",
    "set-cookie": "PHPSESSID=sjus1qucjph83upcjpptf3kd16; path=/",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding,Api_Key,Api_Token,Device_Token,Api_Trx_Token,User_Passport,Transaction_Id",
    "x-content-type-options": "nosniff",
    "x-request-id": "019d1dd3-2ee5-76f5-890b-e23bb9084548"
  },
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
  -H "x-request-id: refund_refund_with_reason_req" \
  -H "x-connector-request-reference-id: refund_refund_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_d5568dc776294699a9f59139",
  "connector_transaction_id": "unknown",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "reason": "customer_requested"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:11:24 GMT
x-request-id: refund_refund_with_reason_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "UNKNOWN",
      "message": "Unknown error occurred"
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, Content-Type, x-readme-api-explorer",
    "access-control-allow-methods": "OPTIONS,GET,POST,PATCH,DELETE",
    "cache-control": "no-store, no-cache, must-revalidate",
    "content-security-policy": "default-src 'none'",
    "content-type": "application/json; charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:11:24 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "pragma": "no-cache",
    "server": "Apache",
    "set-cookie": "PHPSESSID=odcsapantlpg5l14f3qu4q8dc1; path=/",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding,Api_Key,Api_Token,Device_Token,Api_Trx_Token,User_Passport,Transaction_Id",
    "x-content-type-options": "nosniff",
    "x-request-id": "019d1dd3-5ca6-7efb-bf96-0abf64c7aec9"
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
