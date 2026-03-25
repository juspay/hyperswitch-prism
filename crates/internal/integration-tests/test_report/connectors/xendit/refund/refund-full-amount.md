# Connector `xendit` / Suite `refund` / Scenario `refund_full_amount`

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
  "merchant_transaction_id": "mti_b13dad7b96fd4870b7c4a59e",
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
        "value": "Ava Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "riley.2909@testmail.io"
    },
    "id": "cust_024bf4489024410289ef9941",
    "phone_number": "+15359521994"
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
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "7874 Oak St"
      },
      "line2": {
        "value": "2212 Pine Dr"
      },
      "line3": {
        "value": "1492 Market Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "50453"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.2227@sandbox.example.com"
      },
      "phone_number": {
        "value": "5622503086"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "5343 Market Blvd"
      },
      "line2": {
        "value": "799 Oak Blvd"
      },
      "line3": {
        "value": "566 Pine Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27133"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.9075@testmail.io"
      },
      "phone_number": {
        "value": "7824716699"
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
date: Tue, 24 Mar 2026 05:36:39 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "d060a65b-e857-4b5b-a184-4816a2cca82a",
  "connectorTransactionId": "pr-cfe15f41-ed71-4aab-9904-f62d6421cd1b",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1351a61895054c-BOM",
    "connection": "keep-alive",
    "content-length": "1691",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:35 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "50",
    "rate-limit-reset": "32.122",
    "request-id": "69c222e2000000004c4741714cbfac97",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=FD24Z.rxEEPgqqNEBzaSsEcqK5xW4IDRJy2EP0ulttk-1774330594.2522318-1.0.1.1-btUhh7WKKthsZuQiHi3M03SHje0ICq4Q020qkQMbawuB6DwXyObs3IGLCPgPA8tq86vhayFk4hzb5OpKQEObRgHbq9XpwiPU2bdVM8C1JD0ANpI0gwixYj2fVXK35Cdu; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:35 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1517"
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
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_481980d0fe174fc091cc2adf",
  "connector_transaction_id": "pr-cfe15f41-ed71-4aab-9904-f62d6421cd1b",
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
<summary>Show Response (masked)</summary>

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
date: Tue, 24 Mar 2026 05:36:41 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "rfd-8f1a4fa9-f66f-48d1-9744-aae7d5e8c4b7",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1351c5dada054c-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:41 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "59",
    "rate-limit-reset": "60",
    "request-id": "69c222e7000000003e14772804283c78",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=aUo4cYCTmNJBv0YgKnmFnqcJB0WfhhV_s0cREdm0__w-1774330599.3318756-1.0.1.1-dAOlbgycE7R18LLbAEwYdhLhCeOZ4e0fXBF6j74XzWKY2et43k_S6WQeyDM3PnlIorYJLnQPpOD41YTuoMEK1cgYFq7mIJLg50iFVLFYBbwADhUhJ3hnizoMsJS9HQvq; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:41 GMT",
    "transfer-encoding": "chunked",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "2148"
  },
  "connectorTransactionId": "pr-cfe15f41-ed71-4aab-9904-f62d6421cd1b",
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
