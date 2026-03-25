# Connector `fiuu` / Suite `refund` / Scenario `refund_partial_amount`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `PASS`

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
  "merchant_transaction_id": "mti_29806d1300d147a883a01021",
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
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "alex.9442@example.com"
    },
    "id": "cust_29f84dc693604696b0324855",
    "phone_number": "+19520593495"
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
        "value": "8840 Oak St"
      },
      "line2": {
        "value": "2365 Oak Blvd"
      },
      "line3": {
        "value": "8473 Sunset Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "98580"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.1003@sandbox.example.com"
      },
      "phone_number": {
        "value": "4539010383"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "7946 Pine Blvd"
      },
      "line2": {
        "value": "3605 Pine Dr"
      },
      "line3": {
        "value": "5860 Market Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73456"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6295@sandbox.example.com"
      },
      "phone_number": {
        "value": "5065944978"
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
date: Mon, 23 Mar 2026 18:46:54 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "31270189",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=600",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f99f7fe07ff64-BOM",
    "connection": "keep-alive",
    "content-type": "text/html; charset=UTF-8",
    "date": "Mon, 23 Mar 2026 18:46:53 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "tmpOid=deleted; expires=Thu, 01-Jan-1970 00:00:01 GMT; Max-Age=0",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding,User-Agent",
    "x-content-type-options": "nosniff"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "TK_2902_35512301472677909531"
    }
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
  "merchant_refund_id": "mri_008cbab677d64044b83f686c",
  "connector_transaction_id": "31270189",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 3000,
    "currency": "USD"
  },
  "webhook_url": "https://example.com/payment/webhook"
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
date: Mon, 23 Mar 2026 18:46:54 GMT
x-request-id: refund_refund_partial_amount_req

Response contents:
{
  "connectorRefundId": "81715",
  "status": "REFUND_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=600",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f99fcbd13ff64-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:46:54 GMT",
    "server": "cloudflare",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "vary": "Accept-Encoding,User-Agent",
    "x-content-type-options": "nosniff"
  },
  "connectorTransactionId": "31270189",
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
