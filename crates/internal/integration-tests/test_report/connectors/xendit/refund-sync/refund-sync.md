# Connector `xendit` / Suite `refund_sync` / Scenario `refund_sync`

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
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:49 GMT
x-request-id: refund_sync_refund_sync_req
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
  "merchant_transaction_id": "mti_d65f2cbc6141466a880936e8",
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
        "value": "Liam Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "alex.7432@testmail.io"
    },
    "id": "cust_7891de00d3994df3b4a8357a",
    "phone_number": "+13659647824"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "2146 Sunset Rd"
      },
      "line2": {
        "value": "7318 Sunset Blvd"
      },
      "line3": {
        "value": "1402 Lake Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39548"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "sam.6661@example.com"
      },
      "phone_number": {
        "value": "3056784370"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "2605 Pine Blvd"
      },
      "line2": {
        "value": "9574 Sunset Rd"
      },
      "line3": {
        "value": "42 Sunset Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44924"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.4180@testmail.io"
      },
      "phone_number": {
        "value": "3735538455"
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
date: Tue, 24 Mar 2026 05:36:47 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "93cf2bf2-4dd4-44bf-a315-a7a1d4e82455",
  "connectorTransactionId": "pr-c78655b2-f037-4840-9dfe-5999cf4646d5",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1351efcc10054c-BOM",
    "connection": "keep-alive",
    "content-length": "1690",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:47 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "47",
    "rate-limit-reset": "20.325",
    "request-id": "69c222ee000000001ce86f59550ae70c",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=zSh52u_qkHn1GnjDuMPAdrkWeQ8hx7ubatdLm0oSNU8-1774330606.0417817-1.0.1.1-M9Somw7eotMv_mtO.np2zD039Uq9mgavHMdUiNVTqkXmfu1lRNaW5gDDhWkqhT3MkRE3AIthqr.Ol1hEfiiZWwTQfAwtVrtR3xj3PlhzKfu3AFcYwisXO1YRVO1iGC0t; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:47 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1500"
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
  "merchant_refund_id": "mri_1cc4d013258743a99c966d92",
  "connector_transaction_id": "pr-c78655b2-f037-4840-9dfe-5999cf4646d5",
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
date: Tue, 24 Mar 2026 05:36:48 GMT
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
    "cf-ray": "9e1351faca7e054c-BOM",
    "connection": "keep-alive",
    "content-length": "68",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:47 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "56",
    "rate-limit-reset": "51.528",
    "request-id": "69c222ef0000000025db62481ec088b6",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=hpqDKbHqrqwcInAMnqLEpAFxxDr8otkVb5kyp4JWc7w-1774330607.805175-1.0.1.1-n1xcPWyayBZ6lFM7VJyokYW6rz0wxhqguA6a9CWIfzh1KUHNlHZNEnM9YzMUikkKFJtw8F7Q_Labjmb_80d42uZ3ZNBEjAs7kWcoYtWT2izWyCZeUqKhpIWpyaqfBeZ8; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:47 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "105"
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "pr-c78655b2-f037-4840-9dfe-5999cf4646d5"
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
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:49 GMT
x-request-id: refund_sync_refund_sync_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to deserialize connector response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
