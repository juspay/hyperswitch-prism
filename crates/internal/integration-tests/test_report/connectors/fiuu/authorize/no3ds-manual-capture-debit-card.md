# Connector `fiuu` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

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
  "merchant_transaction_id": "mti_e87d69b3791a4f7a99367a7b",
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
        "value": "Mia Miller"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Brown",
    "email": {
      "value": "casey.5635@testmail.io"
    },
    "id": "cust_4c82cc15df97416897d3b662",
    "phone_number": "+913389775607"
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
        "value": "Miller"
      },
      "line1": {
        "value": "6510 Main St"
      },
      "line2": {
        "value": "462 Sunset Dr"
      },
      "line3": {
        "value": "8522 Sunset Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "95279"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.7971@testmail.io"
      },
      "phone_number": {
        "value": "9505932092"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3622 Pine Ln"
      },
      "line2": {
        "value": "6646 Pine St"
      },
      "line3": {
        "value": "634 Oak Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35258"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1935@sandbox.example.com"
      },
      "phone_number": {
        "value": "8666988203"
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
date: Mon, 23 Mar 2026 18:46:45 GMT
x-request-id: authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "connectorTransactionId": "31270180",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "max-age=600",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f99c51e63ff64-BOM",
    "connection": "keep-alive",
    "content-type": "text/html; charset=UTF-8",
    "date": "Mon, 23 Mar 2026 18:46:45 GMT",
    "expires": "Thu, 19 Nov 1981 08:52:00 GMT",
    "pragma": "no-cache",
    "server": "cloudflare",
    "set-cookie": "tmpOid=deleted; expires=Thu, 01-Jan-1970 00:00:01 GMT; Max-Age=0",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding,User-Agent",
    "x-content-type-options": "nosniff"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "TK_2902_35512301472677909531"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
