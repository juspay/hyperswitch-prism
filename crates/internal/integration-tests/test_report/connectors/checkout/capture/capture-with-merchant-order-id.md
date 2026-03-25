# Connector `checkout` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_e96cbbfff10d409983ee1a9b",
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
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Taylor",
    "email": {
      "value": "alex.4025@example.com"
    },
    "id": "cust_4ff1296c39f7486db1079f5f",
    "phone_number": "+18066373136"
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
        "value": "8426 Sunset Dr"
      },
      "line2": {
        "value": "6896 Main St"
      },
      "line3": {
        "value": "1156 Sunset Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "79340"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9513@testmail.io"
      },
      "phone_number": {
        "value": "8454554451"
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
        "value": "1447 Sunset Dr"
      },
      "line2": {
        "value": "8630 Pine Ave"
      },
      "line3": {
        "value": "2764 Sunset St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96210"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.8019@sandbox.example.com"
      },
      "phone_number": {
        "value": "9927390879"
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
date: Mon, 23 Mar 2026 18:39:14 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_e96cbbfff10d409983ee1a9b",
  "connectorTransactionId": "pay_rk7jsiabuycibaslbawwpg4sce",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "cko-request-id": "2bfcddfb-5ab9-4fc6-8ca9-460921f4771d",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "2062",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:14 GMT",
    "location": "https://api.sandbox.checkout.com/payments/pay_rk7jsiabuycibaslbawwpg4sce",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "networkTransactionId": "263882164232628",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "capturableAmount": "6000",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzdWx0IjoiSSIsImNhcmRfdmFsaWRhdGlvbl9yZXN1bHQiOiJQIn0="
      }
    }
  },
  "connectorFeatureData": {
    "value": "{\"psync_flow\":\"Authorize\"}"
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
  "connector_transaction_id": "pay_rk7jsiabuycibaslbawwpg4sce",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_5fd06952332542fc9e26a2c0",
  "merchant_order_id": "gen_640044",
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
  "connector_feature_data": {
    "value": "{\"psync_flow\":\"Authorize\"}"
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
date: Mon, 23 Mar 2026 18:39:14 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "pay_rk7jsiabuycibaslbawwpg4sce",
  "status": "CHARGED",
  "statusCode": 202,
  "responseHeaders": {
    "cko-request-id": "26f8bab3-9dd1-41e1-b2fc-51cac3472d9a",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "151",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:14 GMT",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "capturedAmount": "6000",
  "connectorFeatureData": {
    "value": "{\"psync_flow\":\"Capture\"}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
