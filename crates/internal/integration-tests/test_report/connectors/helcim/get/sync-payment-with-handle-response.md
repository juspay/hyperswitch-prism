# Connector `helcim` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
  "merchant_transaction_id": "mti_b72baee5e9b44e71873a57ff",
  "amount": {
    "minor_amount": 6107,
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
        "value": "Mia Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Wilson",
    "email": {
      "value": "alex.5138@example.com"
    },
    "id": "cust_8e8d445a96044f8e92ad52cc",
    "phone_number": "+918135298094"
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
        "value": "4920 Sunset Ln"
      },
      "line2": {
        "value": "6967 Market Dr"
      },
      "line3": {
        "value": "8656 Oak Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "65082"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9947@sandbox.example.com"
      },
      "phone_number": {
        "value": "2943417679"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9620 Sunset Blvd"
      },
      "line2": {
        "value": "312 Market Ln"
      },
      "line3": {
        "value": "7216 Lake Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97574"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1229@sandbox.example.com"
      },
      "phone_number": {
        "value": "9769397682"
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
date: Tue, 24 Mar 2026 06:12:47 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "500",
      "message": "internal_server_error",
      "reason": "{\"transactionId\":46054269,\"dateCreated\":\"2026-03-24 00:12:47\",\"cardBatchId\":6226719,\"status\":\"DECLINED\",\"user\":\"Helcim System\",\"type\":\"purchase\",\"amount\":61.07,\"currency\":\"USD\",\"avsResponse\":\"\",\"cvvResponse\":\"\",\"cardType\":\"VI\",\"approvalCode\":\"\",\"cardToken\":\"\",\"cardNumber\":\"4111111111\",\"cardHolderName\":\"Noah Miller\",\"customerCode\":\"CST12095\",\"invoiceNumber\":\"mti_b72baee5e9b44e71873a57ff\",\"warning\":\"\",\"errors\":\"Transaction Declined: Suspected duplicate transaction in the last 5 minutes.\"}"
    }
  },
  "statusCode": 500,
  "responseHeaders": {
    "access-control-allow-headers": "Origin, Content-Type, X-Auth-Token, js-token, user-token, business-id",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Origin, Content-Type, X-Auth-Token, js-token",
    "access-control-max-age": "600",
    "alt-svc": "h3=\":443\"; ma=86400",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1386ae3f6557ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:12:47 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "83",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=9S54vS4DfjLkEXvCx3.kX6ewPHpPUdCxPB4Y4HyPUzU-1774332766.435231-1.0.1.1-K_HMtbehpaMjzvpT2Uh1c6h4DKI9bUw0jzOO5ci6j9bhsSjosdlNyhcNtWfn11uS7s9eLDDY67dKs8a8lNJpTdQwIdoeyCnUNoLm8Tn9Xy.J5sSzd4suzLlsZHuAS4O9Z0z8Alqo7xkD4rNtba90zA; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:42:47 GMT",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked"
  },
  "rawConnectorResponse": "***MASKED***"
  },
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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount": {
    "minor_amount": 6107,
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
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:12:47 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "No error code",
      "message": "Failed to retrieve card transaction #auto_generate.",
      "reason": "Failed to retrieve card transaction #auto_generate."
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "access-control-allow-headers": "Origin, Content-Type, X-Auth-Token, js-token, user-token, business-id",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Origin, Content-Type, X-Auth-Token, js-token",
    "access-control-max-age": "600",
    "alt-svc": "h3=\":443\"; ma=86400",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1386b4aef657ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:12:47 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "82",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=n0hV3DafNY5_9lTXQYMxXBUtyYpstx0_XjfGTqNsuRM-1774332767.4651313-1.0.1.1-9yVGM1QSdY.dH2FoiuuLTep0FvsvaqrbkAdz3PyNCzRJMLu0rvE2Qaq_ayNZVRD25mmx5B5MJ.fK8mxzULiGKY5hzI.H5Hn7EgOqBFd5CAwOBnssu45WEVJmitN6msa5SqsEhd3XKPFfbRP23..SdA; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:42:47 GMT",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
