# Connector `xendit` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_a5899aafdbf74d7681ae05c8",
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
    "name": "Noah Taylor",
    "email": {
      "value": "casey.8699@testmail.io"
    },
    "id": "cust_9562518e2b3a4e9e9ddfdab8",
    "phone_number": "+449231083585"
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
        "value": "8373 Oak Dr"
      },
      "line2": {
        "value": "2576 Sunset Rd"
      },
      "line3": {
        "value": "6550 Main Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "14154"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "sam.6021@sandbox.example.com"
      },
      "phone_number": {
        "value": "2064624655"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "2718 Main St"
      },
      "line2": {
        "value": "3737 Pine Dr"
      },
      "line3": {
        "value": "2713 Market Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96588"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.6992@example.com"
      },
      "phone_number": {
        "value": "5236406479"
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
date: Tue, 24 Mar 2026 05:36:31 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "2a410a88-1364-4211-8ec3-03f6f3a1d946",
  "connectorTransactionId": "pr-185e5c0d-bf4e-4bff-9909-e034b635ae3d",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13518958c6054c-BOM",
    "connection": "keep-alive",
    "content-length": "1688",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:31 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "51",
    "rate-limit-reset": "36.722",
    "request-id": "69c222dd000000001fe227db8e94fa38",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=RlkuEjbfKw8vfeIz4mUQwpGBPM0yW_3cYSBUpea28nk-1774330589.6512563-1.0.1.1-Q_krq8RctQYNfgwt308mxDQruD021npAJ6QqF70NKH_afKhg3.n1..NPwmM3F_UGtdu2wP0NCGWLhIM5zR4MVY0NgQ7lX6IUuSffLSWqZK5PaAzPDvaD0s29iYNOMHXy; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:31 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1492"
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
  "connector_transaction_id": "pr-185e5c0d-bf4e-4bff-9909-e034b635ae3d",
  "amount_to_capture": {
    "minor_amount": 1500000,
    "currency": "IDR"
  },
  "merchant_capture_id": "mci_0cfbe9b8ff04428a8e076710",
  "merchant_order_id": "gen_901565",
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
date: Tue, 24 Mar 2026 05:36:34 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e135198786f054c-BOM",
    "connection": "keep-alive",
    "content-length": "1626",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:34 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "57",
    "rate-limit-reset": "54.789",
    "request-id": "69c222e00000000062fbe7e065f6d582",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=ReYEFNqZzk.pHyxhR6ObMM2h81JVIC6OKeIraJqgRnc-1774330592.0731375-1.0.1.1-L0hxlz6jppIMfu3T2EhsFUFPK2Obne7RQpmeSqw9c1227eE6GtXq3.bJlXNDdlpJoVtK4z5t9Agc0q68DqZxhLpUGdAffxq4v8lYZa6CfK7AgbuPss13zq0JoyWjrapB; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:34 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1866"
  },
  "merchantCaptureId": "24c4c4a0-179b-4d75-ad97-d6fdfa7c715a",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
