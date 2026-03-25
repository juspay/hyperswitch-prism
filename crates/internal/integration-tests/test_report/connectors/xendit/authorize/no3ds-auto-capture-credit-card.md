# Connector `xendit` / Suite `authorize` / Scenario `no3ds_auto_capture_credit_card`

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
  "merchant_transaction_id": "mti_081ebc1f67074378b696bd34",
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
        "value": "Noah Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "alex.2034@testmail.io"
    },
    "id": "cust_9eb4c1ae2ccd45f483e45e15",
    "phone_number": "+11368621478"
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
        "value": "7255 Sunset Rd"
      },
      "line2": {
        "value": "2230 Main Rd"
      },
      "line3": {
        "value": "279 Oak Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "19239"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "sam.6209@testmail.io"
      },
      "phone_number": {
        "value": "6281501260"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3450 Pine Blvd"
      },
      "line2": {
        "value": "8032 Pine Blvd"
      },
      "line3": {
        "value": "6387 Pine Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "28095"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "riley.2308@sandbox.example.com"
      },
      "phone_number": {
        "value": "9899267184"
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
date: Tue, 24 Mar 2026 05:36:08 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "0ae5a167-cad5-4156-b7bd-d22f1d09ff28",
  "connectorTransactionId": "pr-2b7a9cde-4d22-43f4-b56a-91098df97fa5",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1350f7baed054c-BOM",
    "connection": "keep-alive",
    "content-length": "1700",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:08 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "59",
    "rate-limit-reset": "60",
    "request-id": "69c222c600000000693741cb0ac47ad1",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=CLAsy7OwEofP.iYEXopFxgWtdcMDZmODx2tjvHxrIzE-1774330566.3609028-1.0.1.1-EPmCd5ge_RSfPVX.MtLjvDacelIl3Bvkrgu2tfKVK3o4DZwO9B8Bd9oCtQfA67toTA2KqPZzJ.zgwOD.7oRxsaxrEDWsxLFjGxveFZBiiHRVhU9cYMow0TRxy47kJ6do; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:08 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1724"
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
