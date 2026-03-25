# Connector `xendit` / Suite `refund_sync` / Scenario `refund_sync_with_reason`

- Service: `RefundService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:52 GMT
x-request-id: refund_sync_refund_sync_with_reason_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to deserialize connector response
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

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
  "merchant_transaction_id": "mti_5aaa843ce4684075865d3bf6",
  "amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "alex.9286@testmail.io"
    },
    "id": "cust_2ecf698c85d047349fb48bf3",
    "phone_number": "+448163688807"
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
        "value": "Smith"
      },
      "line1": {
        "value": "8134 Main Blvd"
      },
      "line2": {
        "value": "638 Lake Ave"
      },
      "line3": {
        "value": "1991 Oak Ln"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96163"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "jordan.3673@testmail.io"
      },
      "phone_number": {
        "value": "7683013800"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8349 Market Blvd"
      },
      "line2": {
        "value": "1475 Sunset Ave"
      },
      "line3": {
        "value": "3650 Market Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "78448"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "jordan.9150@sandbox.example.com"
      },
      "phone_number": {
        "value": "9810360053"
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
date: Tue, 24 Mar 2026 05:36:50 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "d91d7ba5-6a81-48e6-b501-5fac7236dba1",
  "connectorTransactionId": "pr-7c3d708f-33e1-4772-8dad-6b6622c6b4aa",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1352034ff6054c-BOM",
    "connection": "keep-alive",
    "content-length": "1700",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:50 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "46",
    "rate-limit-reset": "17.215",
    "request-id": "69c222f10000000048761386ab0762c1",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=val2EYtuM9fk3RQcaCuLtk1GuzcEkCx6nYSmOiZQ_Uc-1774330609.161203-1.0.1.1-h048TG9HYN72dJDUQrqoEvKg_4X5RrNECbmLSe31C.QyEbQeCbEA5AfaDkbTsA1cdKPwG._xghQEaWt2a92XGDw73VQsifOBj299mDrlP8ODB2l7RM8eoRmcEx_wQKlD; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:50 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1548"
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
<summary>2. refund(refund_full_amount) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_3a814f1a06b643dca4421783",
  "connector_transaction_id": "pr-7c3d708f-33e1-4772-8dad-6b6622c6b4aa",
  "payment_amount": 1500000,
  "refund_amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:51 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "API_VALIDATION_ERROR",
      "message": "no captures found"
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13520e8fb0054c-BOM",
    "connection": "keep-alive",
    "content-length": "68",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:51 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "55",
    "rate-limit-reset": "48.369",
    "request-id": "69c222f30000000077ad89143278f359",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=9ounu_hCby7R2buvUYmymYYkrI_A5361iABxcEoyjHI-1774330610.9680772-1.0.1.1-AVheA8.5y3oLicZNfiNXNYZhQvr7hnpilfTVGDp7Cru494s4RAIANLWOSkY_fgtVShZ9J7ljJOnHoqta4DWCfRuOOjUt0xKnZr7qdkpqjRwoR_6qjRq1FfcnBzTeMHSS; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:51 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "87"
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
  -H "x-request-id: refund_sync_refund_sync_with_reason_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "pr-7c3d708f-33e1-4772-8dad-6b6622c6b4aa",
  "refund_reason": "customer_requested"
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
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:52 GMT
x-request-id: refund_sync_refund_sync_with_reason_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to deserialize connector response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
