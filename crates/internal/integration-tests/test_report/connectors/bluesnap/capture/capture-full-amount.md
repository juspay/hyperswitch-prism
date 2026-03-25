# Connector `bluesnap` / Suite `capture` / Scenario `capture_full_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `PASS`

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
  "merchant_transaction_id": "mti_4ce1cbf361c24c218f5b1fb1",
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
        "value": "Ethan Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "morgan.8079@sandbox.example.com"
    },
    "id": "cust_1c30aa2631694b7fa8c29f64",
    "phone_number": "+913652525158"
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
        "value": "9494 Lake Blvd"
      },
      "line2": {
        "value": "5126 Sunset Ln"
      },
      "line3": {
        "value": "5289 Sunset Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "14356"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7783@testmail.io"
      },
      "phone_number": {
        "value": "1794998480"
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
        "value": "6216 Lake St"
      },
      "line2": {
        "value": "6617 Main St"
      },
      "line3": {
        "value": "202 Lake Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "82772"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7952@sandbox.example.com"
      },
      "phone_number": {
        "value": "4657296350"
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
date: Mon, 23 Mar 2026 18:32:15 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "1087579064",
  "connectorTransactionId": "1087579064",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f847f4d835644-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:32:15 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=aO73hh7KKiLbcbzLiGGlOTPtv4tCo3MM4FW73FY6.8Q-1774290735-1.0.1.1-.NdhO30iE08op.B8iIsVGj67wiAJwWlPapg0e0Z.1DXecA03cPFO.XgrOkTh6msLcVCiHY_UEar4jT.v4fL7JGF3eLyke7mQuYFFOcH1qlg; path=/; expires=Mon, 23-Mar-26 19:02:15 GMT; domain=.bluesnap.com; HttpOnly; Secure",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding"
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
  -H "x-request-id: capture_capture_full_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "1087579064",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_449ef30e49764913a53c01f6",
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
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_full_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:32:16 GMT
x-request-id: capture_capture_full_amount_req

Response contents:
{
  "connectorTransactionId": "1087579064",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f8488dcf85644-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:32:16 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=q6iMe6Upcc7cDggiWMeH4H30Nz5qekBs4ih7cRqL4WM-1774290736-1.0.1.1-i2thULxk_7_usnFBvtfbRiXpyoj85MDX37DgYHe8xb7dWg519JWpojdlDuWPM1rN6SPZtXH7DvZIQnSjPvbUfU2ed5wPIySHDqJ9gvAUULk; path=/; expires=Mon, 23-Mar-26 19:02:16 GMT; domain=.bluesnap.com; HttpOnly; Secure",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding"
  },
  "merchantCaptureId": "1087579064",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
