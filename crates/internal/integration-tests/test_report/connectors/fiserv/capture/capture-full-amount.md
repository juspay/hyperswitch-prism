# Connector `fiserv` / Suite `capture` / Scenario `capture_full_amount`

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
  "merchant_transaction_id": "mti_6fedee11a7b34ebda10f2455",
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
        "value": "Ethan Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "jordan.5412@sandbox.example.com"
    },
    "id": "cust_9f160c3dceca4acfbd49420e",
    "phone_number": "+11675260575"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "3083 Market Blvd"
      },
      "line2": {
        "value": "8078 Sunset Ln"
      },
      "line3": {
        "value": "9884 Lake Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "46084"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9569@testmail.io"
      },
      "phone_number": {
        "value": "3869883964"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4591 Market Ave"
      },
      "line2": {
        "value": "7867 Market Blvd"
      },
      "line3": {
        "value": "2763 Lake Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "16007"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8660@example.com"
      },
      "phone_number": {
        "value": "2689855217"
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
date: Tue, 24 Mar 2026 07:41:48 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "CHG019eaf841d5a6a1dd0c3fd402aa9342b87",
  "connectorTransactionId": "8eaeded96bae4452911219808744e261",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "8eaeded96bae4452911219808744e261",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "3485",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:41:48 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338107; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:41:47 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338108131",
    "targetserversentstarttimestamp": "1774338107686",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "e2594f6c-5377-48a1-823b-0bb501f3c25f8179.1",
    "x-vcap-request-id": "8eaeded9-6bae-4452-9112-19808744e261",
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
  -H "x-request-id: capture_capture_full_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "8eaeded96bae4452911219808744e261",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_27c5bb9a0310467584078a4b",
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
x-connector-request-reference-id: capture_capture_full_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:41:49 GMT
x-request-id: capture_capture_full_amount_req

Response contents:
{
  "connectorTransactionId": "234c3a4a510c4f18864a078c8a3b11d4",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "234c3a4a510c4f18864a078c8a3b11d4",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2355",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:41:49 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338108; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:41:48 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338109143",
    "targetserversentstarttimestamp": "1774338108932",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "ab24088f-88b3-474c-b331-12b2e5fb01b59774.1",
    "x-vcap-request-id": "234c3a4a-510c-4f18-864a-078c8a3b11d4",
    "x-xss-protection": "1; mode=block"
  },
  "merchantCaptureId": "CHG019eaf841d5a6a1dd0c3fd402aa9342b87",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
