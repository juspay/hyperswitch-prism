# Connector `adyen` / Suite `void` / Scenario `void_authorized_payment`

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
  "merchant_transaction_id": "mti_0113d2d9fcf24a8d87051563",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "5101180000000007"
      },
      "card_exp_month": {
        "value": "03"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "737"
      },
      "card_holder_name": {
        "value": "Liam Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Taylor",
    "email": {
      "value": "alex.9479@testmail.io"
    },
    "id": "cust_13953f411ad2409994dcc064",
    "phone_number": "+449749179925"
  },
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 900,
    "screen_width": 1440,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -330,
    "language": "en-US"
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
        "value": "6915 Main Dr"
      },
      "line2": {
        "value": "1136 Oak Rd"
      },
      "line3": {
        "value": "26 Pine Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "47157"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9407@testmail.io"
      },
      "phone_number": {
        "value": "3872013061"
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
        "value": "2442 Lake Rd"
      },
      "line2": {
        "value": "8959 Sunset Blvd"
      },
      "line3": {
        "value": "4155 Market Ln"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "63289"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.3857@sandbox.example.com"
      },
      "phone_number": {
        "value": "5739102823"
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
date: Tue, 24 Mar 2026 03:26:24 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_0113d2d9fcf24a8d87051563",
  "connectorTransactionId": "G9PB4KSTGNDXML65",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:24 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "DHFPL9PDRMN6DN65",
    "set-cookie": "JSESSIONID=335865132EACB5271EDEA2583C081000; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-c1491eaf49e1ee2ce346740ed0546e2d-89c6bf53ae852206-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
  },
  "networkTransactionId": "5YRUD622D0324",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "authCode": "004839"
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
  "connector_transaction_id": "G9PB4KSTGNDXML65",
  "merchant_void_id": "mvi_1ef6a5e7cccf4a949c1e5b1d",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8",
    "user_agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 900,
    "screen_width": 1440,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -330
  },
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
date: Tue, 24 Mar 2026 03:26:24 GMT
x-request-id: void_void_authorized_payment_req

Response contents:
{
  "connectorTransactionId": "G9PB4KSTGNDXML65",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:24 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "M8SWL9PDRMN6DN65",
    "set-cookie": "JSESSIONID=B2E99699724CDF7305C4EB0D0862F2CF; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-89bd71694b0375372f1ee4926968fbb3-6e451490b316c393-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
  },
  "merchantVoidId": "mvi_1ef6a5e7cccf4a949c1e5b1d",
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
