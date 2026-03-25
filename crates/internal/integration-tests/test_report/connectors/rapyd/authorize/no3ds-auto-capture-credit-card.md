# Connector `rapyd` / Suite `authorize` / Scenario `no3ds_auto_capture_credit_card`

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
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_66acefe123cc44b1ba72c5498d42ee3b",
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
        "value": "Liam Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "alex.6517@sandbox.example.com"
    },
    "id": "cust_20f61769a98c41e3a2ef2f97e739d89a",
    "phone_number": "+914444368054"
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
        "value": "Brown"
      },
      "line1": {
        "value": "4231 Pine Blvd"
      },
      "line2": {
        "value": "8321 Market Dr"
      },
      "line3": {
        "value": "4584 Pine Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "13617"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6140@sandbox.example.com"
      },
      "phone_number": {
        "value": "8556800860"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "322 Lake Ln"
      },
      "line2": {
        "value": "8762 Oak Ln"
      },
      "line3": {
        "value": "1097 Main Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "11265"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8638@example.com"
      },
      "phone_number": {
        "value": "3173576053"
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
<summary>Show Response (masked)</summary>

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
date: Mon, 23 Mar 2026 16:29:21 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_66acefe123cc44b1ba72c5498d42ee3b",
  "connectorTransactionId": "payment_cf3dec2fb062be7c386e1ed156775d00",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ed0778ff7c8e8-MRS",
    "connection": "keep-alive",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 16:29:21 GMT",
    "etag": "W/\"908-hXdwCOnmfr3WW51N+VTr2olkoQs\"",
    "server": "cloudflare",
    "set-cookie": "_cfuvid=gF6S9ErKXBfn0npLZZROLVSD3ciZlE_qQnLUjgMZP1Q-1774283359.921447-1.0.1.1-2QcfwBB4nK.C7AbnY0o1O32x7gsgdsHOkL7CH.pp0ZU; HttpOnly; SameSite=None; Secure; Path=/; Domain=rapyd.net",
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "transfer-encoding": "chunked"
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
