# Connector `checkout` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
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
  "merchant_transaction_id": "mti_0466445196d5487991b85c11",
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
        "value": "Noah Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "casey.4720@sandbox.example.com"
    },
    "id": "cust_29bf066952ef43aa92706a05",
    "phone_number": "+917623018566"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "1946 Oak Rd"
      },
      "line2": {
        "value": "2726 Market St"
      },
      "line3": {
        "value": "3561 Market St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "43584"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.7878@testmail.io"
      },
      "phone_number": {
        "value": "1516001326"
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
        "value": "1670 Pine Ave"
      },
      "line2": {
        "value": "1649 Main Blvd"
      },
      "line3": {
        "value": "3892 Pine Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74490"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1851@testmail.io"
      },
      "phone_number": {
        "value": "4360331773"
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
date: Mon, 23 Mar 2026 18:39:23 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_0466445196d5487991b85c11",
  "connectorTransactionId": "pay_arqm75ab4lmy5gfe24krycydxy",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "cko-request-id": "cca04f94-3ab0-4f42-beeb-654eba1fb30d",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "2060",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:22 GMT",
    "location": "https://api.sandbox.checkout.com/payments/pay_arqm75ab4lmy5gfe24krycydxy",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "networkTransactionId": "282067137896674",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "capturedAmount": "6000",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzdWx0IjoiSSIsImNhcmRfdmFsaWRhdGlvbl9yZXN1bHQiOiJQIn0="
      }
    }
  },
  "connectorFeatureData": {
    "value": "{\"psync_flow\":\"Capture\"}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. refund(refund_full_amount) — PASS</summary>

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
  "merchant_refund_id": "mri_2c7528972a1a487caab93e81",
  "connector_transaction_id": "pay_arqm75ab4lmy5gfe24krycydxy",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_feature_data": {
    "value": "{\"psync_flow\":\"Capture\"}"
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
date: Mon, 23 Mar 2026 18:39:23 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "act_msi375a2czne5dvz76vdihfuwa",
  "status": "REFUND_SUCCESS",
  "statusCode": 202,
  "responseHeaders": {
    "cko-request-id": "df42b1df-4117-9930-a7a6-643827e8821e",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "194",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:23 GMT",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "connectorTransactionId": "pay_arqm75ab4lmy5gfe24krycydxy",
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
  "connector_transaction_id": "pay_arqm75ab4lmy5gfe24krycydxy",
  "refund_id": "act_msi375a2czne5dvz76vdihfuwa",
  "connector_feature_data": {
    "value": "{\"psync_flow\":\"Capture\"}"
  }
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
content-type: application/grpc
date: Mon, 23 Mar 2026 18:39:23 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "merchantRefundId": "act_msi375a2czne5dvz76vdihfuwa",
  "connectorRefundId": "act_msi375a2czne5dvz76vdihfuwa",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "1216",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:23 GMT",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "connectorTransactionId": "pay_arqm75ab4lmy5gfe24krycydxy",
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
