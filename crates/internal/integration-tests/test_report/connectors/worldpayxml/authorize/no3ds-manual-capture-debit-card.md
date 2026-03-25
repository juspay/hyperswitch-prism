# Connector `worldpayxml` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_debit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_329c993d46d3421180d2a115",
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
        "value": "Ava Taylor"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Smith",
    "email": {
      "value": "sam.9615@testmail.io"
    },
    "id": "cust_b5308115bf4c4105bd9d2995",
    "phone_number": "+919913719799"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "7 Main St"
      },
      "line2": {
        "value": "180 Pine St"
      },
      "line3": {
        "value": "171 Lake Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "62235"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.4141@example.com"
      },
      "phone_number": {
        "value": "2595205481"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "1405 Pine Ave"
      },
      "line2": {
        "value": "9993 Main Dr"
      },
      "line3": {
        "value": "6041 Lake St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "58210"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9775@example.com"
      },
      "phone_number": {
        "value": "5913314462"
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
  "description": "No3DS manual capture card payment (debit)",
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:11:21 GMT
x-request-id: authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "mti_329c993d46d3421180d2a115",
  "connectorTransactionId": "mti_329c993d46d3421180d2a115",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=0, no-cache, no-store",
    "connection": "keep-alive",
    "content-security-policy-report-only": "default-src https: data: 'unsafe-eval' 'unsafe-inline'; object-src 'none'; report-uri https://secure.worldpay.com/public/CspReport",
    "content-type": "text/plain",
    "date": "Tue, 24 Mar 2026 07:11:21 GMT",
    "expires": "Tue, 24 Mar 2026 07:11:21 GMT",
    "p3p": "CP=\"NON\"",
    "pragma": "no-cache",
    "set-cookie": "machine=0a854016;Secure;path=/",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "vary": "Accept-Encoding",
    "x-cnection": "close",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "371923",
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
