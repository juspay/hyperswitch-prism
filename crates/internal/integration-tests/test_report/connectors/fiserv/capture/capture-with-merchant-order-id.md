# Connector `fiserv` / Suite `capture` / Scenario `capture_with_merchant_order_id`

- Service: `PaymentService/Capture`
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
  "merchant_transaction_id": "mti_5dec67e83926407b8f4eb780",
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
        "value": "Mia Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Miller",
    "email": {
      "value": "sam.6961@sandbox.example.com"
    },
    "id": "cust_2ab33b1fb122498e93805020",
    "phone_number": "+442696406135"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "2498 Main Blvd"
      },
      "line2": {
        "value": "6908 Oak Ave"
      },
      "line3": {
        "value": "6803 Lake Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85957"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.9934@testmail.io"
      },
      "phone_number": {
        "value": "1220154884"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8485 Market Ave"
      },
      "line2": {
        "value": "7615 Sunset Ln"
      },
      "line3": {
        "value": "5078 Pine Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85891"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8222@example.com"
      },
      "phone_number": {
        "value": "9471845095"
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
date: Tue, 24 Mar 2026 07:41:56 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "CHG014110716aed088298e91e0f5e38c047ae",
  "connectorTransactionId": "418fd32dbe3541c38665aaa4b01ed988",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "418fd32dbe3541c38665aaa4b01ed988",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "3485",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:41:56 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338115; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:41:55 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338116279",
    "targetserversentstarttimestamp": "1774338115606",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "82d14837-9d0a-4dc5-96fc-3d033686d2b09977.1",
    "x-vcap-request-id": "418fd32d-be35-41c3-8665-aaa4b01ed988",
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "418fd32dbe3541c38665aaa4b01ed988",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_03855650ec6842d8b339c1c2",
  "merchant_order_id": "gen_870321",
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
    "value": "{\"terminal_id\":\"10000001\"}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:41:57 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "60ca74c2955741c799aded87bb096334",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "60ca74c2955741c799aded87bb096334",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2355",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:41:57 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338116; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:41:56 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338117295",
    "targetserversentstarttimestamp": "1774338117085",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "82d14837-9d0a-4dc5-96fc-3d033686d2b09979.1",
    "x-vcap-request-id": "60ca74c2-9557-41c7-99ad-ed87bb096334",
    "x-xss-protection": "1; mode=block"
  },
  "merchantCaptureId": "CHG014110716aed088298e91e0f5e38c047ae",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
