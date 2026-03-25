# Connector `payload` / Suite `void` / Scenario `void_authorized_payment`

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
  "merchant_transaction_id": "mti_df2ea8d54009431f9f8a766b170e9726",
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
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "alex.5393@sandbox.example.com"
    },
    "id": "cust_497d1afd46da468cb5df934b5be048c2",
    "phone_number": "+449722946341"
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
        "value": "Brown"
      },
      "line1": {
        "value": "6950 Pine Blvd"
      },
      "line2": {
        "value": "3566 Main St"
      },
      "line3": {
        "value": "216 Main Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "58318"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4178@sandbox.example.com"
      },
      "phone_number": {
        "value": "6187071663"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3706 Pine Rd"
      },
      "line2": {
        "value": "9608 Sunset Ln"
      },
      "line3": {
        "value": "6941 Sunset Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "22432"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9951@sandbox.example.com"
      },
      "phone_number": {
        "value": "5717174697"
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
date: Mon, 23 Mar 2026 16:23:01 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "PL-66Q-K9N-5B5",
  "connectorTransactionId": "txn_3fCKm7HVD2gVfxR0cXZO2",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, must-revalidate",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ec7377a3e3e33-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:23:01 GMT",
    "pragma": "no-cache",
    "server": "cloudflare",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzdWx0Ijoic3RyZWV0X2FuZF96aXAifQ=="
      }
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
  -H "x-request-id: void_void_authorized_payment_req" \
  -H "x-connector-request-reference-id: void_void_authorized_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "txn_3fCKm7HVD2gVfxR0cXZO2",
  "merchant_void_id": "mvi_9bed4660ca1b4f329b561dd117eb2ade",
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
x-connector-request-reference-id: void_void_authorized_payment_ref
x-merchant-id: test_merchant
x-request-id: void_void_authorized_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:23:02 GMT
x-request-id: void_void_authorized_payment_req

Response contents:
{
  "connectorTransactionId": "txn_3fCKm7HVD2gVfxR0cXZO2",
  "status": "VOIDED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, must-revalidate",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ec73e3ee83e33-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:23:02 GMT",
    "pragma": "no-cache",
    "server": "cloudflare",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "merchantVoidId": "PL-66Q-K9N-5B5",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
