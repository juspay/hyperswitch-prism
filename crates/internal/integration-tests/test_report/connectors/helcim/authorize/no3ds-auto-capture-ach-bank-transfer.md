# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_ach_bank_transfer`

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
  -H "x-request-id: authorize_no3ds_auto_capture_ach_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_ach_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_db75f166f0454d10867fb858",
  "amount": {
    "minor_amount": 6101,
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
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Johnson",
    "email": {
      "value": "morgan.2935@sandbox.example.com"
    },
    "id": "cust_647442937b094ad9b31eeae6",
    "phone_number": "+15334393167"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "3081 Pine Blvd"
      },
      "line2": {
        "value": "3699 Main Blvd"
      },
      "line3": {
        "value": "2559 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33502"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7936@example.com"
      },
      "phone_number": {
        "value": "3221024671"
      },
      "phone_country_code": "+1"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "2191 Lake Dr"
      },
      "line2": {
        "value": "991 Oak Blvd"
      },
      "line3": {
        "value": "780 Oak St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34393"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3265@example.com"
      },
      "phone_number": {
        "value": "5513450496"
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
  "description": "No3DS ACH bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_ach_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_ach_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:11:11 GMT
x-request-id: authorize_no3ds_auto_capture_ach_bank_transfer_req

Response contents:
{
  "merchantTransactionId": "mti_db75f166f0454d10867fb858",
  "connectorTransactionId": "46054241",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Origin, Content-Type, X-Auth-Token, js-token, user-token, business-id",
    "access-control-allow-methods": "GET, POST, PUT, PATCH, DELETE, OPTIONS",
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "Origin, Content-Type, X-Auth-Token, js-token",
    "access-control-max-age": "600",
    "alt-svc": "h3=\":443\"; ma=86400",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13844c3f0057ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:11 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "100",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=QnN5RuhrSTuh71j9KbPCLbndR60mggXsoiXYbOdQfm4-1774332668.8382113-1.0.1.1-2410S7A5Bi9E.suIjk1SJKdWNeO77178YDNHidYhdbN90YCqo_5bfwdzJ6I7XMVKiWKlyWR1yG1i0e6OmYPncI2I.pIC.PIf0MVFm4kmQS_admSmpcmg.CoV78hhkvA.uNUsOcXuu5UXFY3WJe_vOA; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:11 GMT",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
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
