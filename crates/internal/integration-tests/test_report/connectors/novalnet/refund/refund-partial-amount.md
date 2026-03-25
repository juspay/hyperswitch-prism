# Connector `novalnet` / Suite `refund` / Scenario `refund_partial_amount`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

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
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:47:54 GMT
x-request-id: refund_refund_partial_amount_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to deserialize connector response
```

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

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
  "merchant_transaction_id": "mti_7b56fc1f22414682abc1582e",
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
        "value": "Ethan Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "morgan.4558@example.com"
    },
    "id": "cust_9fece18e22dd4a25a4c1481b",
    "phone_number": "+913336300705"
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
        "value": "Brown"
      },
      "line1": {
        "value": "263 Sunset Dr"
      },
      "line2": {
        "value": "5969 Sunset Ave"
      },
      "line3": {
        "value": "7223 Pine Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35798"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1220@sandbox.example.com"
      },
      "phone_number": {
        "value": "7180980608"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "6440 Sunset Rd"
      },
      "line2": {
        "value": "6509 Oak Dr"
      },
      "line3": {
        "value": "5331 Lake Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "48161"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3709@example.com"
      },
      "phone_number": {
        "value": "1515169619"
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
date: Tue, 24 Mar 2026 01:47:54 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "content-length": "287",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "content-type": "application/json",
    "cross-origin-embedder-policy": "require-corp",
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-resource-policy": "same-origin",
    "date": "Tue, 24 Mar 2026 01:47:53 GMT",
    "origin-agent-cluster": "?1",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "referrer-policy": "no-referrer",
    "server": "Apache",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "x-content-type-options": "nosniff",
    "x-dns-prefetch-control": "off",
    "x-download-options": "noopen",
    "x-frame-options": "SAMEORIGIN",
    "x-permitted-cross-domain-policies": "none",
    "x-xss-protection": "0"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/5cfbd19b4176949c47244b6bb9821f76",
      "method": "HTTP_METHOD_GET"
    }
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
  "merchant_refund_id": "mri_e5d3eace08614fefb93886b7",
  "connector_transaction_id": "auto_generate",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 3000,
    "currency": "USD"
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
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 01:47:54 GMT
x-request-id: refund_refund_partial_amount_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Failed to deserialize connector response
```

</details>


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
