# Connector `rapyd` / Suite `PaymentService/Authorize` / Scenario `Debit Card | No 3DS | Manual Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_manual_capture_debit_card`
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
  -H "x-request-id: PaymentService/Authorize_no3ds_manual_capture_debit_card_req" \
  -H "x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_dd4063fdf5d34b9eb5f531a4",
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
        "value": "Emma Taylor"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "jordan.6832@sandbox.example.com"
    },
    "id": "cust_a61ceb1c85414840a9d625ae",
    "phone_number": "+441865353440",
    "connector_customer_id": ""
  },
  "session_token": ***MASKED***"
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6856 Main Blvd"
      },
      "line2": {
        "value": "7530 Market Blvd"
      },
      "line3": {
        "value": "3836 Market Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "51499"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8123@sandbox.example.com"
      },
      "phone_number": {
        "value": "8015773365"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "968 Pine Ave"
      },
      "line2": {
        "value": "3119 Sunset St"
      },
      "line3": {
        "value": "6309 Pine Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42904"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2717@example.com"
      },
      "phone_number": {
        "value": "3034384712"
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
  "locale": "en-US",
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
  "order_details": []
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
x-connector-request-reference-id: PaymentService/Authorize_no3ds_manual_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: PaymentService/Authorize_no3ds_manual_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:20 GMT
x-request-id: PaymentService/Authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "mti_dd4063fdf5d34b9eb5f531a4",
  "connectorTransactionId": "payment_be003817e73a16b40d2146e2847d091c",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9ebbd37b9b2cb630-BOM",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 13 Apr 2026 16:25:20 GMT",
    "etag": "W/\"8bf-QAN1DcqlfNrnTNn/qUoGwZ1RWqQ\"",
    "server": "cloudflare",
    "set-cookie": ***MASKED***"
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
  },
  "state": {
    "connectorCustomerId": ""
  },
  "rawConnectorResponse": "***MASKED***",
  "rawConnectorRequest": "***MASKED***"


Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
