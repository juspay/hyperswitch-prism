# Connector `bluesnap` / Suite `get` / Scenario `sync_payment_with_handle_response`

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
  "merchant_transaction_id": "mti_2edf743b9f274be295f197ef",
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
    "name": "Ethan Miller",
    "email": {
      "value": "sam.3935@sandbox.example.com"
    },
    "id": "cust_aff6c26a18034fd2b04fcecf",
    "phone_number": "+445925934322"
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
        "value": "Miller"
      },
      "line1": {
        "value": "7673 Lake Ave"
      },
      "line2": {
        "value": "7050 Main Ave"
      },
      "line3": {
        "value": "7405 Pine Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18924"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7544@example.com"
      },
      "phone_number": {
        "value": "2946369476"
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
        "value": "8642 Lake Dr"
      },
      "line2": {
        "value": "5710 Market Dr"
      },
      "line3": {
        "value": "9428 Pine Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "65881"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6845@example.com"
      },
      "phone_number": {
        "value": "2383195528"
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
date: Mon, 23 Mar 2026 18:33:00 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "1087579308",
  "connectorTransactionId": "1087579308",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f8593ca8e5644-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:33:00 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=ivDrXKdAba0UrdV1xPZTWwdab1YAqVDtWAtKArBTVD8-1774290780-1.0.1.1-HsbnWn80t8hUFtRmzsmhA_FEAc.3qX3ED56G6t0wHA4RbGxh.FB7ZcvRLp58gL.pWqzAOFLd62eq7YKytC67Dgdlczq9HUrjLaIcpAaphi8; path=/; expires=Mon, 23-Mar-26 19:03:00 GMT; domain=.bluesnap.com; HttpOnly; Secure",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding"
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
  "connector_transaction_id": "1087579308",
  "amount": {
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
date: Mon, 23 Mar 2026 18:33:02 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "connectorTransactionId": "1087579308",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f85a1cb275644-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:33:00 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=c3rJTzvNHYPHfGV5DA55LtuS3xncYbEENmBSQakJcUo-1774290780-1.0.1.1-tIhAn0PrC34GN0thX5f8rOAOgF1O7OParM0m5pmMBFvoauy6QBboOn_MtVlHCg52ee2MIB2vk.hXgtlb_RuiYhwQgZg0pEd58IZjtsJmC.U; path=/; expires=Mon, 23-Mar-26 19:03:00 GMT; domain=.bluesnap.com; HttpOnly; Secure",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "merchantTransactionId": "1087579308"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
