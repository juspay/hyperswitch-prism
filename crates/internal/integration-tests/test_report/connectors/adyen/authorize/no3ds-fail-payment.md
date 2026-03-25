# Connector `adyen` / Suite `authorize` / Scenario `no3ds_fail_payment`

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
  "merchant_transaction_id": "mti_8bdc498990f74df1b7cf5fb3",
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
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Wilson",
    "email": {
      "value": "jordan.2724@testmail.io"
    },
    "id": "cust_6f00bb6c08cb4be4bd34f6be",
    "phone_number": "+448117047771"
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
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8856 Main St"
      },
      "line2": {
        "value": "2243 Main Blvd"
      },
      "line3": {
        "value": "7545 Market St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "72691"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.8814@example.com"
      },
      "phone_number": {
        "value": "2997478524"
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
        "value": "554 Pine Rd"
      },
      "line2": {
        "value": "7537 Main Dr"
      },
      "line3": {
        "value": "8651 Main Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27804"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9494@example.com"
      },
      "phone_number": {
        "value": "2238241729"
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
date: Tue, 24 Mar 2026 03:26:15 GMT
x-request-id: authorize_no3ds_fail_payment_req

Response contents:
{
  "merchantTransactionId": "BRLNB6SDF9Q94875",
  "connectorTransactionId": "BRLNB6SDF9Q94875",
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "message": "DECLINED Expiry Incorrect",
      "networkDetails": {
        "adviceCode": "24",
        "errorMessage": "DECLINED Expiry Incorrect"
      }
    },
    "connectorDetails": {
      "code": "2",
      "message": "Refused",
      "reason": "Refused"
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:15 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "D7VSQRQ2J6HM7L75",
    "set-cookie": "JSESSIONID=A9C9E2C93877A008B0282F2028D6F0C0; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-5c526959a84106c443f8452070dec309-db2eae85a2203230-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
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
