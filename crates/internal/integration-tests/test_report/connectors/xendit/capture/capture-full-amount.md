# Connector `xendit` / Suite `capture` / Scenario `capture_full_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
  "merchant_transaction_id": "mti_40e370edae3341399284838b",
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
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "casey.8421@testmail.io"
    },
    "id": "cust_26961336dc23412e8818dd75",
    "phone_number": "+912859632361"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4896 Lake Dr"
      },
      "line2": {
        "value": "8323 Lake Ave"
      },
      "line3": {
        "value": "2960 Lake Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "37724"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "riley.9353@example.com"
      },
      "phone_number": {
        "value": "6609852146"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3716 Lake Blvd"
      },
      "line2": {
        "value": "9254 Sunset St"
      },
      "line3": {
        "value": "5161 Main Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "88315"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "jordan.5262@sandbox.example.com"
      },
      "phone_number": {
        "value": "4872208542"
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
date: Tue, 24 Mar 2026 05:36:26 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "3cc3b756-b475-45ed-984b-24bbd467df14",
  "connectorTransactionId": "pr-84dfeddc-768f-49f8-bd15-1ada3693d2a0",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13516c69ab054c-BOM",
    "connection": "keep-alive",
    "content-length": "1699",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:26 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "53",
    "rate-limit-reset": "41.352",
    "request-id": "69c222d9000000001de6dc020aed2239",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=KNf8uHBJ2SeZ7yIljKX.eidaRX7uanusalrk82csgdk-1774330585.0212138-1.0.1.1-6sptqrSDvxQ99hXv.i3yMKwagxA4vDNQwCET3DRTHKINkWgXhh3uukffI.BCSattJmD1ura23AKfP6qPceWFCv.HqNSPYMVnHmZOZkfAyYij3QPox6Yh5aAt4RC0lLkK; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:26 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1583"
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
  "connector_transaction_id": "pr-84dfeddc-768f-49f8-bd15-1ada3693d2a0",
  "amount_to_capture": {
    "minor_amount": 1500000,
    "currency": "IDR"
  },
  "merchant_capture_id": "mci_b4846adfda9d466c90768e2f",
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
date: Tue, 24 Mar 2026 05:36:27 GMT
x-request-id: capture_capture_full_amount_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "PAYMENT_REQUEST_ALREADY_FAILED",
      "message": "The payment_request_id provided has already been processed and has failed."
    }
  },
  "statusCode": 409,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e135177cf67054c-BOM",
    "connection": "keep-alive",
    "content-length": "135",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:27 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "59",
    "rate-limit-reset": "60",
    "request-id": "69c222da00000000692bf1fb5bbc6c5a",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=UUllYTtMHtkgeonh35Xa8OpwxN1Hikx44tiC7KnmVAQ-1774330586.849166-1.0.1.1-ax3QhAvjXtPAkdlOGx9EwopVeoqox5wiEoO1.gNbkL7dFtcptQul.LyyauO7d6OR8JTgWNeFaIqZtpQbgbtd4hzifjYxC.tdEeFszRJCGGCDgbNQxc1d_qLA61y0B77u; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:27 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "177"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
