# Connector `fiuu` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

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
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_fdb863fafa2544fc85e738f5",
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
        "value": "Noah Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "sam.7002@testmail.io"
    },
    "id": "cust_f5c8acbe315b4e9f8ceaca42",
    "phone_number": "+919948858378"
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
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "7669 Lake Rd"
      },
      "line2": {
        "value": "3222 Sunset Ave"
      },
      "line3": {
        "value": "4462 Main St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "10841"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.2117@testmail.io"
      },
      "phone_number": {
        "value": "3020700834"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "534 Pine Ave"
      },
      "line2": {
        "value": "2528 Sunset Dr"
      },
      "line3": {
        "value": "9200 Oak Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "19269"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.3233@example.com"
      },
      "phone_number": {
        "value": "2720516526"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
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
x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:46:46 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "31270181",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=600",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f99c90c15ff64-BOM",
    "connection": "keep-alive",
    "content-type": "text/html; charset=UTF-8",
    "date": "Mon, 23 Mar 2026 18:46:46 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "nbtime=1774291606",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding,User-Agent",
    "x-content-type-options": "nosniff"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://sandbox.merchant.razer.com/RMS/demobank/direct_server_intermediate.php",
      "method": "HTTP_METHOD_POST",
      "formFields": {
        "paRes": "ZXlKUVdGOVdSVkpUU1U5T0lqb2lNUzR4SWl3aVVGaGZWRkpCVGxOQlExUkpUMDVmVkZsUVJTSTZJa0ZWVkZNaUxDSlFXRjlRVlZKRFNFRlRSVjlKUkNJNklqTXhNamN3TVRneElpd2lVRmhmVUVGT0lqb2lOREV4TVRFeE1URXhNVEV4TVRFeE1TSXNJbEJZWDBOV1ZqSWlPaUk1T1RraUxDSlFXRjlGV0ZCSlVsa2lPaUl3T0RNd0lpd2lVRmhmVFVWU1EwaEJUbFJmU1VRaU9pSXdPQ0lzSWxCWVgxQlZVa05JUVZORlgwRk5UMVZPVkNJNklqWXdMakF3SWl3aVVGaGZVRlZTUTBoQlUwVmZSRVZUUTFKSlVGUkpUMDRpT2lKd1lYbHRaVzUwT2lBek1USTNNREU0TVNJc0lsQllYMUJWVWtOSVFWTkZYMFJCVkVVaU9pSXlOREF6TWpBeU5pQXdNam8wTmpvME5pSXNJbEJZWDFKRlJpSTZJaUlzSWxCWVgwTlZVMVJQVFY5R1NVVk1SREVpT2lJaUxDSlFXRjlEVlZOVVQwMWZSa2xGVEVReUlqb2lJaXdpVUZoZlExVlRWRTlOWDBaSlJVeEVNeUk2SWlJc0lsQllYME5WVTFSUFRWOUdTVVZNUkRRaU9pSWlMQ0pRV0Y5RFZWTlVUMDFmUmtsRlRFUTFJam9pSWl3aVVGaGZVMGxISWpvaUlpd2liV1Z5WTJoaGJuUkpSQ0k2SWxOQ1gzcDFjbWxqYUdka2NIQWlmUT09"
      }
    }
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
