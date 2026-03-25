# Connector `xendit` / Suite `authorize` / Scenario `no3ds_auto_capture_debit_card`

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
  -H "x-request-id: authorize_no3ds_auto_capture_debit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_f3821cb5be2d4d8a892dc212",
  "amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
        "value": "Mia Taylor"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "sam.8428@example.com"
    },
    "id": "cust_2a87fcb6de224f96bf558171",
    "phone_number": "+914236831238"
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
        "value": "Brown"
      },
      "line1": {
        "value": "8128 Market Ln"
      },
      "line2": {
        "value": "2 Pine Ave"
      },
      "line3": {
        "value": "9616 Oak St"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "48582"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.3105@testmail.io"
      },
      "phone_number": {
        "value": "7662030044"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "4107 Market Ave"
      },
      "line2": {
        "value": "2543 Oak Ln"
      },
      "line3": {
        "value": "3931 Sunset Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80264"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "jordan.9674@sandbox.example.com"
      },
      "phone_number": {
        "value": "5864938133"
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
  "description": "No3DS auto capture card payment (debit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:10 GMT
x-request-id: authorize_no3ds_auto_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "d7e37648-9c8d-447a-8a7e-c90880d38c2f",
  "connectorTransactionId": "pr-83ec662e-1d7b-4f6e-a956-b9dd0aa2f482",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13510449eb054c-BOM",
    "connection": "keep-alive",
    "content-length": "1701",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:10 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "58",
    "rate-limit-reset": "58.011",
    "request-id": "69c222c800000000132b8223d2c9236d",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=RfSql18MuJUPypSuDuBJBzAhM9usIcNLWP7bB8qgv6o-1774330568.362905-1.0.1.1-KpFWg4ONxwqaduG4V32y7h8RAmlarRZF1SIgMCj9NVgoWG5v7OClJvzcw4mL6ZJ1R9HH9chDISlHju00NcqFxBCSHGUHc69N2kWUHq_zu7DNu1egepEZx6BXd.jNII4r; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:10 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1641"
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
