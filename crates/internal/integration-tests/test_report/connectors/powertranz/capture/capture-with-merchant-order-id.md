# Connector `powertranz` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_bf81a23a2c6e4d2686128617",
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
        "value": "Mia Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Miller",
    "email": {
      "value": "sam.6033@example.com"
    },
    "id": "cust_04b1f4425a5c417a89574320",
    "phone_number": "+16611130587"
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
        "value": "3187 Sunset Ln"
      },
      "line2": {
        "value": "4842 Main Dr"
      },
      "line3": {
        "value": "3287 Market Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "87058"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2465@sandbox.example.com"
      },
      "phone_number": {
        "value": "2164953991"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4371 Lake Ave"
      },
      "line2": {
        "value": "5663 Main Ln"
      },
      "line3": {
        "value": "7288 Pine Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99618"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8286@example.com"
      },
      "phone_number": {
        "value": "5867930441"
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
date: Tue, 24 Mar 2026 07:06:32 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "2abbe63b-2f00-4f2f-bfcf-6a491a42cb9f",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d56b0b2647e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:32 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=616atqvaxaNNgqLqF3GkLLcNZe8p.F79bNS2PezqdQE-1774335991.5283139-1.0.1.1-Jfy3uvb_b.7MGbhi1RL.eQMl6isDlEoB25wJVMnAQLA_S5ImZmttlFD3dvmTKOLN_v6yGfTK80_jkGvZXewOs.Wgy3fKroP6JK.wB4bUy7DNvzBhymrUQXo_JOFbqK_G; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:32 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "2abbe63b-2f00-4f2f-bfcf-6a491a42cb9f",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_bfc0a23653a6447eb5e06b2a",
  "merchant_order_id": "gen_105976",
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
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:06:34 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "71c6217d-107e-471a-98f5-8c28e7d8405e",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d573aa8947e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:34 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=cHxCXwutuis0y1YZwp9bIWmxPJnf4UdTJUft61wnYMA-1774335992.9081929-1.0.1.1-WPlU9JIeE5haRAg8O63dbEUmS5dlrMCgGYTI9l2MxkbVXp.6QSt_YBTna9eziI9u3xJULqc1P4XqHz_moC4K3m0LWduLOsz0mYDsZ7ncV90j1tRMHLs_AehXueUadD2D; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:34 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
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
