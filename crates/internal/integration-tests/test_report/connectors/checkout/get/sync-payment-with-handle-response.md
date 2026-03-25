# Connector `checkout` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
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
  "merchant_transaction_id": "mti_294d080067ed44fd9d3df383",
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
        "value": "Emma Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "jordan.8769@testmail.io"
    },
    "id": "cust_983fce92cd634a5fa5009b79",
    "phone_number": "+911775074611"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "1119 Oak Dr"
      },
      "line2": {
        "value": "6676 Lake Ave"
      },
      "line3": {
        "value": "6997 Main Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "30470"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8613@example.com"
      },
      "phone_number": {
        "value": "4766466086"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3359 Pine Ln"
      },
      "line2": {
        "value": "9372 Main St"
      },
      "line3": {
        "value": "6384 Oak Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18999"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4913@example.com"
      },
      "phone_number": {
        "value": "3301409856"
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
date: Mon, 23 Mar 2026 18:39:27 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_294d080067ed44fd9d3df383",
  "connectorTransactionId": "pay_5b7g2hqbwlmyveqdp5tdfb2zpq",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "cko-request-id": "2b6181ca-dbf9-9763-a0cc-d1f642c9c11f",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "2062",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:27 GMT",
    "location": "https://api.sandbox.checkout.com/payments/pay_5b7g2hqbwlmyveqdp5tdfb2zpq",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "networkTransactionId": "544595506038355",
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
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "pay_5b7g2hqbwlmyveqdp5tdfb2zpq",
  "amount": {
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
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:39:28 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "connectorTransactionId": "pay_5b7g2hqbwlmyveqdp5tdfb2zpq",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "1904",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:28 GMT",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "networkTransactionId": "544595506038355",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzdWx0IjoiSSIsImNhcmRfdmFsaWRhdGlvbl9yZXN1bHQiOiJQIn0="
      }
    }
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "merchantTransactionId": "mti_294d080067ed44fd9d3df383"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
