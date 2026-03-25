# Connector `powertranz` / Suite `refund` / Scenario `refund_full_amount`

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
  "merchant_transaction_id": "mti_ee846be02fe0473b86bbae7b",
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
        "value": "Noah Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "morgan.4035@example.com"
    },
    "id": "cust_d97c5c283d704cb9a54da14a",
    "phone_number": "+442816983491"
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
        "value": "3905 Pine St"
      },
      "line2": {
        "value": "2074 Oak St"
      },
      "line3": {
        "value": "124 Lake Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "57965"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4344@sandbox.example.com"
      },
      "phone_number": {
        "value": "7368290660"
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
        "value": "9695 Lake Ave"
      },
      "line2": {
        "value": "8835 Sunset Ln"
      },
      "line3": {
        "value": "1588 Main Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "70423"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9944@testmail.io"
      },
      "phone_number": {
        "value": "5891013689"
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
date: Tue, 24 Mar 2026 07:06:46 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "934da073-5e13-43e8-a9ac-4e6ff9e7ad31",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d5beee7e47e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:46 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=ANzv.6PGqnOguUX4O93d5JNUiR132TcZTz3R6iewSOg-1774336004.9439468-1.0.1.1-qwR_eaOVjrbtFBpzfD.fPTsLmwLrz7UC0nt5QkmINkuJLZydYVvlMzRoY5T.MREM1HSvoAQ_GcU5dKi1atkJfTbv3noWtJgIeIy5349fnEKIwFzfuIri6qSYlYvLei9i; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:46 GMT",
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
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_27c4d7426a924207917a7401",
  "connector_transaction_id": "934da073-5e13-43e8-a9ac-4e6ff9e7ad31",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
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
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:06:47 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "459ea410-a4fc-437c-81d2-9fdc818ff3ce",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d5c7ed6247e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:47 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=wteoPe7Y1Kn6A760yxpjsIDtpd8eMk2jRvbHeWre8uc-1774336006.387586-1.0.1.1-xW39fezj.xez2SqYFSDgJtvt8OlbLlGkIWLH8wr.Wd0j3rfEXlgNn01moYLxJfd6EhuFRn7vQcayPpF76fPsf2KnK1YGy7p1Jrns84VYpfrpUXdvmTNMClLnT7tDA.Ts; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:47 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "934da073-5e13-43e8-a9ac-4e6ff9e7ad31",
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
