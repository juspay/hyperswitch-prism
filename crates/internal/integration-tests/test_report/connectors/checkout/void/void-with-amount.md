# Connector `checkout` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
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
  "merchant_transaction_id": "mti_a2b8ca3b083642a594b4bb02",
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
        "value": "Ethan Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Miller",
    "email": {
      "value": "sam.2513@testmail.io"
    },
    "id": "cust_a6717e80c9404ddeb7cb100f",
    "phone_number": "+441095909150"
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
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "2115 Main Dr"
      },
      "line2": {
        "value": "9948 Oak Rd"
      },
      "line3": {
        "value": "1695 Oak Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "93094"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4602@example.com"
      },
      "phone_number": {
        "value": "9938563492"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5231 Sunset Ave"
      },
      "line2": {
        "value": "7787 Oak Ave"
      },
      "line3": {
        "value": "6992 Main Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "23857"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8816@testmail.io"
      },
      "phone_number": {
        "value": "7430935812"
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
date: Mon, 23 Mar 2026 18:39:16 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_a2b8ca3b083642a594b4bb02",
  "connectorTransactionId": "pay_yxrfxyqbabkijeu4hdhh3rzpqu",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "cko-request-id": "74b864c6-ef9c-4399-a4c7-048a1a33c3cf",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "2074",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:16 GMT",
    "location": "https://api.sandbox.checkout.com/payments/pay_yxrfxyqbabkijeu4hdhh3rzpqu",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "networkTransactionId": "182083912961933",
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
  -H "x-request-id: void_void_with_amount_req" \
  -H "x-connector-request-reference-id: void_void_with_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "pay_yxrfxyqbabkijeu4hdhh3rzpqu",
  "merchant_void_id": "mvi_9a8dc1d547834f169ede6dff",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_424460",
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
  },
  "cancellation_reason": "requested_by_customer"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Cancel an authorized payment before capture. Releases held funds back to
// customer, typically used when orders are cancelled or abandoned.
rpc Void ( .types.PaymentServiceVoidRequest ) returns ( .types.PaymentServiceVoidResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: void_void_with_amount_ref
x-merchant-id: test_merchant
x-request-id: void_void_with_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:39:17 GMT
x-request-id: void_void_with_amount_req

Response contents:
{
  "connectorTransactionId": "act_5r3ub7z33pwudhjklp2t3pmuie",
  "status": "VOIDED",
  "statusCode": 202,
  "responseHeaders": {
    "cko-request-id": "884a7c76-3e84-49cf-8d83-6305ed294f8e",
    "cko-version": "1.1683.0+8b0fbe7",
    "connection": "keep-alive",
    "content-length": "196",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:39:17 GMT",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
