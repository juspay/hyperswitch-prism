# Connector `rapyd` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
```

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
  "merchant_transaction_id": "mti_119abd046d724d2f9b65521096d01694",
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
        "value": "Mia Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Smith",
    "email": {
      "value": "jordan.9940@testmail.io"
    },
    "id": "cust_21514bb772e94188a587034d0176227e",
    "phone_number": "+911939886709"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "2762 Oak Ave"
      },
      "line2": {
        "value": "6161 Market Blvd"
      },
      "line3": {
        "value": "1692 Pine Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32413"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7597@example.com"
      },
      "phone_number": {
        "value": "6696309678"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "1501 Market Rd"
      },
      "line2": {
        "value": "4436 Market Rd"
      },
      "line3": {
        "value": "8562 Pine St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91377"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1652@testmail.io"
      },
      "phone_number": {
        "value": "4194259668"
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
date: Mon, 23 Mar 2026 16:29:39 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_119abd046d724d2f9b65521096d01694",
  "connectorTransactionId": "payment_6e95083b5930228cd20377ac1a9edaa4",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ed0eb0f13e1e8-MRS",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 16:29:39 GMT",
    "etag": "W/\"8c5-N5bkYehgU4dOnOVmXpFHXt/l8vI\"",
    "server": "cloudflare",
    "set-cookie": "_cfuvid=5jLI7Nz_qZDkfiRWMGpv5CU_lWcHy51FUosPvWssidA-1774283378.4049232-1.0.1.1-eNZnrnSLpNHE.WIDmhNjV2A5DWQHqEDJ1sF8Pc8ChpU; HttpOnly; SameSite=None; Secure; Path=/; Domain=rapyd.net",
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
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
  -H "x-request-id: void_void_with_amount_req" \
  -H "x-connector-request-reference-id: void_void_with_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "payment_6e95083b5930228cd20377ac1a9edaa4",
  "merchant_void_id": "mvi_824da650d99c4715a6453f35be3d4c12",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_982563",
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
date: Mon, 23 Mar 2026 16:29:39 GMT
x-request-id: void_void_with_amount_req

Response contents:
{
  "connectorTransactionId": "payment_6e95083b5930228cd20377ac1a9edaa4",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ed0f22e70e1e8-MRS",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 16:29:39 GMT",
    "etag": "W/\"8c5-3vvbi/2PS3KgYDUxbehCTwwgJX0\"",
    "server": "cloudflare",
    "set-cookie": "_cfuvid=pt9NCCNiERVpWWFaith5Qrp6fH7CbJg_S796UWMzOtI-1774283379.5448828-1.0.1.1-Zt3PR8xgmeAjaLsJV59DCyJJ11icGGVR6fqPQQD.TdY; HttpOnly; SameSite=None; Secure; Path=/; Domain=rapyd.net",
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "merchantVoidId": "mti_119abd046d724d2f9b65521096d01694",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
