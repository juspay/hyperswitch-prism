# Connector `fiserv` / Suite `authorize` / Scenario `no3ds_fail_payment`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_fail_payment_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_fail_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c81db579ea4f4f67902c2e7d",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000000000000002"
      },
      "card_exp_month": {
        "value": "01"
      },
      "card_exp_year": {
        "value": "35"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Emma Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Brown",
    "email": {
      "value": "casey.4289@example.com"
    },
    "id": "cust_80b639ae3e8a40a1afe460f5",
    "phone_number": "+13621703671"
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
        "value": "Johnson"
      },
      "line1": {
        "value": "5897 Main Ave"
      },
      "line2": {
        "value": "3745 Pine Ave"
      },
      "line3": {
        "value": "7012 Market Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "21830"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.4480@example.com"
      },
      "phone_number": {
        "value": "2907790094"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5085 Oak Dr"
      },
      "line2": {
        "value": "6044 Sunset Ln"
      },
      "line3": {
        "value": "308 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "69335"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8127@testmail.io"
      },
      "phone_number": {
        "value": "8438618941"
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
  "description": "No3DS fail payment flow",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_fail_payment_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_fail_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:41:44 GMT
x-request-id: authorize_no3ds_fail_payment_req

Response contents:
{
  "merchantTransactionId": "CHG01fa11497f23128a530189bd3fbfe61c52",
  "connectorTransactionId": "460cd1a9b56c4c369fe3c0de46c34200",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "460cd1a9b56c4c369fe3c0de46c34200",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "4044",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:41:42 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338101; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:41:41 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338102721",
    "targetserversentstarttimestamp": "1774338102067",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "e2594f6c-5377-48a1-823b-0bb501f3c25f8177.1",
    "x-vcap-request-id": "460cd1a9-b56c-4c36-9fe3-c0de46c34200",
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


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
