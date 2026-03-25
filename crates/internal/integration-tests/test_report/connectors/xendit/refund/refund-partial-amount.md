# Connector `xendit` / Suite `refund` / Scenario `refund_partial_amount`

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
  "merchant_transaction_id": "mti_69ba9da6eb0a4d578d1b484c",
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
        "value": "Ethan Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "alex.2271@testmail.io"
    },
    "id": "cust_a5a7e115932f4f418bee5f79",
    "phone_number": "+914552249316"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "8400 Sunset Ave"
      },
      "line2": {
        "value": "6608 Pine St"
      },
      "line3": {
        "value": "2539 Market Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "86299"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "jordan.2989@sandbox.example.com"
      },
      "phone_number": {
        "value": "5269497988"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5279 Oak Ln"
      },
      "line2": {
        "value": "6495 Main Rd"
      },
      "line3": {
        "value": "8535 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "87291"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "morgan.4049@sandbox.example.com"
      },
      "phone_number": {
        "value": "4492701826"
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
date: Tue, 24 Mar 2026 05:36:43 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "93c2d005-87fa-4e29-8a48-b1a9629e4d02",
  "connectorTransactionId": "pr-b5ec20d8-da3d-4d6c-93bc-01e8e3f0ae81",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1351d4dc62054c-BOM",
    "connection": "keep-alive",
    "content-length": "1702",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:43 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "49",
    "rate-limit-reset": "24.634",
    "request-id": "69c222e9000000000e0a5dbd3c1b2ccd",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=9zQkc95q5S_O9TDgfm3iVbVHn6.BjT4YVKSipoqhOI8-1774330601.7375777-1.0.1.1-mu8QDFI7789aV420I2HkhvauDNf0W6x7suVpftXAxQo2Pgi5wN2AxF00BELl69zRurkZLeasualNtdxxooMOvsL9NBuzUdVEkFRVdhai.9gvxcBMX.Fjme2.I5F59zTY; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:43 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1542"
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
  -H "x-request-id: refund_refund_partial_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_869790001e1540ec9806c444",
  "connector_transaction_id": "pr-b5ec20d8-da3d-4d6c-93bc-01e8e3f0ae81",
  "payment_amount": 1500000,
  "refund_amount": {
    "minor_amount": 750000,
    "currency": "IDR"
  }
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
x-connector-request-reference-id: refund_refund_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:43 GMT
x-request-id: refund_refund_partial_amount_req

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
    "cf-ray": "9e1351e00ac0054c-BOM",
    "connection": "keep-alive",
    "content-length": "68",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:43 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "58",
    "rate-limit-reset": "55.816",
    "request-id": "69c222eb000000004ac131fa18a52037",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=hrmtEjKYV2WYn11nlMYHS5KTWMykgLTf7BkVvH0LU1c-1774330603.5222268-1.0.1.1-_PPd_Fx1cPZ8FEM7XaVTiUyGlfnLNzLir.w1gFOkd11g9oJcwwudpxsHzItv28PjQLAEvnvUIFrbPcbW.PJAMHkKt4KHxz06sBezlEfbG2yP9gPFncilVT5IiqWH11QD; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:43 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "100"
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
