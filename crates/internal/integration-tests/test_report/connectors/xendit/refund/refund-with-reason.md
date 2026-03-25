# Connector `xendit` / Suite `refund` / Scenario `refund_with_reason`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
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
  "merchant_transaction_id": "mti_b6b331aaa0f24bcfb0c760ab",
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
        "value": "Ethan Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Brown",
    "email": {
      "value": "alex.9791@testmail.io"
    },
    "id": "cust_06e7dc42df134933bbd62bbe",
    "phone_number": "+17269512402"
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
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3454 Oak Blvd"
      },
      "line2": {
        "value": "4756 Main Dr"
      },
      "line3": {
        "value": "4054 Sunset Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56610"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "morgan.6187@testmail.io"
      },
      "phone_number": {
        "value": "5890986987"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3940 Market Rd"
      },
      "line2": {
        "value": "8697 Sunset Dr"
      },
      "line3": {
        "value": "7261 Market Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41373"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "morgan.7633@testmail.io"
      },
      "phone_number": {
        "value": "8391803538"
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
date: Tue, 24 Mar 2026 05:36:45 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "6a6a7c60-bcfe-4972-8f01-f4283a47e866",
  "connectorTransactionId": "pr-fe9a606c-1c1a-4668-b656-1acd10a7bf0c",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1351e24c02054c-BOM",
    "connection": "keep-alive",
    "content-length": "1695",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:45 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "48",
    "rate-limit-reset": "22.491",
    "request-id": "69c222eb000000003b2ac37bb290c26f",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=RFBQUe1kCpqJBFBm5O5eGgvj3xF1PpTBpTutAWwWDDY-1774330603.8821027-1.0.1.1-3.tafHZVaSfxP0K08._Yv_tyNcsP9ijMGGY8jL4X0M2g6Ee8Mi.8yJXjlaZ0qinQMEW.Ui6iBeappL6.0Y1U4T1wY04y7QfZN98MchRlftMIQzU8xCcO4fqjAdN9AjEw; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:45 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1540"
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
  -H "x-request-id: refund_refund_with_reason_req" \
  -H "x-connector-request-reference-id: refund_refund_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_75992cde0c2b4cfbb02025e2",
  "connector_transaction_id": "pr-fe9a606c-1c1a-4668-b656-1acd10a7bf0c",
  "payment_amount": 1500000,
  "refund_amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
date: Tue, 24 Mar 2026 05:36:45 GMT
x-request-id: refund_refund_with_reason_req

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
    "cf-ray": "9e1351ed8aa0054c-BOM",
    "connection": "keep-alive",
    "content-length": "68",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:45 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "57",
    "rate-limit-reset": "53.658",
    "request-id": "69c222ed0000000010be6e6c66b47c00",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=BGzqpX1GJji7vrdyR1nLiYU2mRVL6YI0l0Lv6WO2W.I-1774330605.6815906-1.0.1.1-_zKmVCHsCaJzYU0ABYXALzMiKM2vBQz1wyV6NoJSPpgl7HUJn1DLIlzWRJltMcLXr_9E3OY.csJ_G32U4DHXwDO4d.PPhp.fa8h5jSFrDVLl22dUG4ZzgGXbpAwnLwit; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:45 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "98"
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


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
