# Connector `adyen` / Suite `authorize` / Scenario `no3ds_auto_capture_afterpay_clearpay`

- Service: `PaymentService/Authorize`
- PM / PMT: `afterpay_clearpay` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_a05eb47a00e54878bad1edad",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "afterpay_clearpay": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "jordan.9305@testmail.io"
    },
    "id": "cust_2e343665b08741b08b1454b4",
    "phone_number": "+447351768680"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "7706 Pine St"
      },
      "line2": {
        "value": "2758 Oak Dr"
      },
      "line3": {
        "value": "9011 Sunset Ln"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34448"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2391@example.com"
      },
      "phone_number": {
        "value": "9626784282"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "3089 Sunset Rd"
      },
      "line2": {
        "value": "6619 Sunset Ave"
      },
      "line3": {
        "value": "1514 Market Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97817"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6364@sandbox.example.com"
      },
      "phone_number": {
        "value": "8647208995"
      },
      "phone_country_code": "+1"
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
  "description": "No3DS auto capture Afterpay/Clearpay payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_afterpay_clearpay_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:26:08 GMT
x-request-id: authorize_no3ds_auto_capture_afterpay_clearpay_req

Response contents:
{
  "merchantTransactionId": "ZX9CGXX4Z5SK8B75",
  "connectorTransactionId": "ZX9CGXX4Z5SK8B75",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:08 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "H578PRQ2J6HM7L75",
    "set-cookie": "JSESSIONID=16822B5931BC658779BBE529F8A8AEF1; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-46d6a433c8da36ef9f3d2eb1581481c4-5b848d5fd0e7e1b6-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://checkoutshopper-test.adyen.com/checkoutshopper/checkoutPaymentRedirect?redirectData=X3XtfGC9%21H4sIAAAAAAAA%2F61Ua2%2FiOBT9L%2Fm6JLUdJ3GQRiMIgb6HUlqhqlLlODeQISSu46Ciiv%2B%2BDoFtt6NZ7UoLQiK%2B5z7O8bl5tyZFA5IvYcQ1t%2FrvFpf5I6g6r0qr77OelSheplGVgtW3eKZBSb7TVSNWVs8SCriGdKBNjCDi28i1CZ0j2id%2BH7E%2FEO4jZHDwJnMF9d9x3q%2B4PDWAyZThwB0GNB6EoTdmsflGAxyb%2BAaUWPFSD4SomrKtdtnUZp5RHEc%2FbgxAQWoaCT3luw2U%2Bi9O4rUx5%2BqYd2HaEI8Goduz%2BKar9G6JRikoxc5UfbgfmWJbbqQxIiCE9r3%2FpktbU%2B2O0Yf7fzP6CTCDDNo52syNzl848iChAUcIPMoClvAUQ8pTk1KpfJmXvJjW8nPWuRew6eyOXPrnN8F14Bmk5MUM6qbQX6BPizCaLBb0ybu%2FYsMDVMFrA7UepGmuDVlenESENxCNhvlKAYxMbsaLGg4J0rjArkpb5xuoGv0pJlYg1ubIOf0ZaA0bebgBi5LMYyFDdoYFtmnCsZ241LU55ZnnYyrMBwcBdQn1fEZc5vlxPGTeII7d0B9Q88wQI2GEkO8GJIqGCLMhctFwHITjCLtDP%2FIoGSE8HnkYM%2B%2BTyhelhqXiLUNH72SrRXQeR1c%2FHuYvk%2Fg2nl1Ev0FvTyawfGbtW%2Fq6UeWDKszJSmtZ95%2FPns9OfOtVJSUoWxtJHZ7uoHREtfkl%2FnFwdO7sUPX7Uubpt39YiBoK43ZIh60Zvxrx5WC8Y4eWguJCd6PHZoiWHHwArivBi1YHKO0ucZ2XRwfLJnG2xFnM766D%2BeX86fEqiAJGnGk4vUkwk7fN4PZuFZHxXE%2Fc4j6h6Xqb%2FHwcN3eyrl8WO3hFnVTddn4VS1ZK88KpDYekenNOJDqlmvpDnOez77paQ%2FkNIeLUHgE3KLICk23Iw8w4iVU%2B%2BflqfrVcFWvh5S7CG9NcJZwy63d3BW98Iwvo2slO%2F%2BezDvv%2F69CztLmHvFye9qp9NG9gs5Lt5iBkUz%2F1OXVdwVLu%2BpCFmZsSSLDHMGVYUJsQjAAZ6ycs8BF3bYSt%2FX7%2FJ2qkdmzMBQAAjhDVALqgsbieFuR%2BnyXwUSqDFstSqz2Zq8GqJengsJY%3D",
      "method": "HTTP_METHOD_GET",
      "formFields": {
        "redirectData": "X3XtfGC9!H4sIAAAAAAAA/61Ua2/iOBT9L/m6JLUdJ3GQRiMIgb6HUlqhqlLlODeQISSu46Ciiv++DoFtt6NZ7UoLQiK+5z7O8bl5tyZFA5IvYcQ1t/rvFpf5I6g6r0qr77OelSheplGVgtW3eKZBSb7TVSNWVs8SCriGdKBNjCDi28i1CZ0j2id+H7E/EO4jZHDwJnMF9d9x3q+4PDWAyZThwB0GNB6EoTdmsflGAxyb+AaUWPFSD4SomrKtdtnUZp5RHEc/bgxAQWoaCT3luw2U+i9O4rUx5+qYd2HaEI8Goduz+Kar9G6JRikoxc5UfbgfmWJbbqQxIiCE9r3/pktbU+2O0Yf7fzP6CTCDDNo52syNzl848iChAUcIPMoClvAUQ8pTk1KpfJmXvJjW8nPWuRew6eyOXPrnN8F14Bmk5MUM6qbQX6BPizCaLBb0ybu/YsMDVMFrA7UepGmuDVlenESENxCNhvlKAYxMbsaLGg4J0rjArkpb5xuoGv0pJlYg1ubIOf0ZaA0bebgBi5LMYyFDdoYFtmnCsZ241LU55ZnnYyrMBwcBdQn1fEZc5vlxPGTeII7d0B9Q88wQI2GEkO8GJIqGCLMhctFwHITjCLtDP/IoGSE8HnkYM++TyhelhqXiLUNH72SrRXQeR1c/HuYvk/g2nl1Ev0FvTyawfGbtW/q6UeWDKszJSmtZ95/Pns9OfOtVJSUoWxtJHZ7uoHREtfkl/nFwdO7sUPX7Uubpt39YiBoK43ZIh60Zvxrx5WC8Y4eWguJCd6PHZoiWHHwArivBi1YHKO0ucZ2XRwfLJnG2xFnM766D+eX86fEqiAJGnGk4vUkwk7fN4PZuFZHxXE/c4j6h6Xqb/HwcN3eyrl8WO3hFnVTddn4VS1ZK88KpDYekenNOJDqlmvpDnOez77paQ/kNIeLUHgE3KLICk23Iw8w4iVU++flqfrVcFWvh5S7CG9NcJZwy63d3BW98Iwvo2slO/+ezDvv/69CztLmHvFye9qp9NG9gs5Lt5iBkUz/1OXVdwVLu+pCFmZsSSLDHMGVYUJsQjAAZ6ycs8BF3bYSt/X7/J2qkdmzMBQAAjhDVALqgsbieFuR+nyXwUSqDFstSqz2Zq8GqJengsJY="
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
