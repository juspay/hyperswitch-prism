# Connector `xendit` / Suite `capture` / Scenario `capture_partial_amount`

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
  "merchant_transaction_id": "mti_663253f67d884344be1f8208",
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
        "value": "Noah Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Smith",
    "email": {
      "value": "morgan.9089@testmail.io"
    },
    "id": "cust_2c3a72a45b0e4896952606a3",
    "phone_number": "+441624305675"
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
        "value": "5087 Sunset Rd"
      },
      "line2": {
        "value": "6725 Sunset Ln"
      },
      "line3": {
        "value": "648 Main Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "93084"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.8031@sandbox.example.com"
      },
      "phone_number": {
        "value": "9119925065"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5003 Main Rd"
      },
      "line2": {
        "value": "6143 Main Blvd"
      },
      "line3": {
        "value": "9637 Lake Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "58928"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.6753@sandbox.example.com"
      },
      "phone_number": {
        "value": "5692259891"
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
date: Tue, 24 Mar 2026 05:36:28 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "ba79562c-e8b9-4139-850a-c8e16c1c6a2e",
  "connectorTransactionId": "pr-38728242-9e74-4694-84a7-83e6085d645e",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13517a78b6054c-BOM",
    "connection": "keep-alive",
    "content-length": "1696",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:28 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "52",
    "rate-limit-reset": "39.102",
    "request-id": "69c222db0000000018be4db89d05c0e7",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=6GqjXI10iitTcZWuzeyLiACM6wwDzPP7CW_ev9mYL9g-1774330587.2748065-1.0.1.1-avH0m3BAgpQT9mWMlJDqav0BkEbDjJGsJIra8Pav3b49VGmWUdbQaqnUSdogn.ggHoCXUuPNyySQ67FJqPlm2EptKM6TrUsl69NoKTYrN9YM4zSG0J29Mf3bgKSOROAk; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:28 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1580"
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
  -H "x-request-id: capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "pr-38728242-9e74-4694-84a7-83e6085d645e",
  "amount_to_capture": {
    "minor_amount": 1500000,
    "currency": "IDR"
  },
  "merchant_capture_id": "mci_095fabb5eb064045a5e38637",
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
x-connector-request-reference-id: capture_capture_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:29 GMT
x-request-id: capture_capture_partial_amount_req

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
    "cf-ray": "9e135185fea2054c-BOM",
    "connection": "keep-alive",
    "content-length": "135",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:29 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "58",
    "rate-limit-reset": "57.647",
    "request-id": "69c222dd00000000687f7ac13ca94342",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=m9VCs8UQ_MvO6wXr9iO2Xr2qKws84A15MolQ.TXWlBk-1774330589.1109498-1.0.1.1-1g1g9FgU31oWVNHC_fMzFifK4Z.hZAqZlZChwiwpnJMAryA35PuHs9laAMrbm9yJ9taXEi1jVwaT426QtTleh7YJKJrHENdL95aKPnJgtEktBfu5jFv9Pz5RkZ5u2LrT; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:29 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "290"
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
