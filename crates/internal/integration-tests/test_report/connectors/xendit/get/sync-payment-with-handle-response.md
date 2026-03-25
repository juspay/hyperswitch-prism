# Connector `xendit` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
  "merchant_transaction_id": "mti_8dd8232a5b3c497fba9e13be",
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
        "value": "Noah Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Taylor",
    "email": {
      "value": "alex.1152@sandbox.example.com"
    },
    "id": "cust_a5d8238cb0d3403f965a1526",
    "phone_number": "+12419691690"
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
        "value": "Brown"
      },
      "line1": {
        "value": "3774 Oak Dr"
      },
      "line2": {
        "value": "4781 Pine Ln"
      },
      "line3": {
        "value": "2565 Market St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83126"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.9333@sandbox.example.com"
      },
      "phone_number": {
        "value": "7749911360"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9611 Oak Ave"
      },
      "line2": {
        "value": "1199 Oak Ln"
      },
      "line3": {
        "value": "440 Oak Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18124"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "sam.9159@sandbox.example.com"
      },
      "phone_number": {
        "value": "8950043834"
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
date: Tue, 24 Mar 2026 05:36:56 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "0222e9bf-8826-4647-885c-613811bd2718",
  "connectorTransactionId": "pr-6685f6b3-329c-40d7-9303-d91e7f0ab38b",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1352241cae054c-BOM",
    "connection": "keep-alive",
    "content-length": "1698",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:56 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "44",
    "rate-limit-reset": "11.96",
    "request-id": "69c222f60000000006abcb1c4eae82b5",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=hBrMVl6nGi5SmXFGj2xo2eAofQflPJwkaVxjwMS85zU-1774330614.4112124-1.0.1.1-EUj5lAYZjZvcmOPVaeNR1WPeHPm1HukKEOf_ESbPYzMdZ0JIaUw2cd5AZ00FEkx2.ddK6X8zMtzgDrvnUcAwZ21fBrjooJdBdfRq60gKgtZfN4xli5BGaKPdMaToFbtu; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:56 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1518"
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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "pr-6685f6b3-329c-40d7-9303-d91e7f0ab38b",
  "amount": {
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
date: Tue, 24 Mar 2026 05:36:56 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "status": "PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13522f6af4054c-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:56 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "58",
    "rate-limit-reset": "57.843",
    "request-id": "69c222f8000000006b65c189b784de4b",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=kSHH6Y35wL1_ZsOIZN.IlnhpR0oxzW8dKzwJjHvcs04-1774330616.2240627-1.0.1.1-sS34_TV7jGFwRjDGydKVBfAV0PoG4bvUeSU0RbcBfp.srbWodY6zmQ.MThYoV8V9rWDf784GqSbmgNsna4.I7TtdyB3ln1ppOXdO42j9dwnvPdwW7vTTlQSGIaNmKSQ1; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:56 GMT",
    "transfer-encoding": "chunked",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "87"
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


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
