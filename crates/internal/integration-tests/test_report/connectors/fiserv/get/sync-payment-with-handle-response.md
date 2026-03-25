# Connector `fiserv` / Suite `get` / Scenario `sync_payment_with_handle_response`

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
  "merchant_transaction_id": "mti_66b3a2493ee7441e866caba5",
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
        "value": "Emma Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "casey.8099@testmail.io"
    },
    "id": "cust_3531aee53eed48d38c8d1d77",
    "phone_number": "+446289515842"
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
        "value": "Miller"
      },
      "line1": {
        "value": "27 Market Ave"
      },
      "line2": {
        "value": "4239 Market Rd"
      },
      "line3": {
        "value": "6945 Pine Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32714"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5137@example.com"
      },
      "phone_number": {
        "value": "1190352628"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4210 Pine St"
      },
      "line2": {
        "value": "7107 Market St"
      },
      "line3": {
        "value": "6687 Main St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "74370"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4957@sandbox.example.com"
      },
      "phone_number": {
        "value": "2476962429"
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
date: Tue, 24 Mar 2026 07:42:23 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "CHG0130c34a5deae38319dd5a561207898ab2",
  "connectorTransactionId": "0f61ec04c3c34b66a828e686f586e270",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "0f61ec04c3c34b66a828e686f586e270",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "3482",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:42:19 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338138; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:42:18 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338139419",
    "targetserversentstarttimestamp": "1774338138992",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "8630f265-5d79-4acb-b455-46127f6132f28543.1",
    "x-vcap-request-id": "0f61ec04-c3c3-4b66-a828-e686f586e270",
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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "0f61ec04c3c34b66a828e686f586e270",
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
date: Tue, 24 Mar 2026 07:42:24 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "connectorTransactionId": "0f61ec04c3c34b66a828e686f586e270",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "48975cc9112442dea0d2d1f913a94f4a",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "4114",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:42:24 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338143; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:42:23 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338144320",
    "targetserversentstarttimestamp": "1774338144014",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "bfeada6d-4e23-4a15-9a09-3a7bb1ad2a199430.1",
    "x-vcap-request-id": "48975cc9-1124-42de-a0d2-d1f913a94f4a",
    "x-xss-protection": "1; mode=block"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "merchantTransactionId": "CHG0130c34a5deae38319dd5a561207898ab2"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
