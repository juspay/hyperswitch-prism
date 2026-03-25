# Connector `helcim` / Suite `authorize` / Scenario `no3ds_auto_capture_bancontact`

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
  -H "x-request-id: authorize_no3ds_auto_capture_bancontact_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_c6a6b211018c4c2eb1cc0f2a",
  "amount": {
    "minor_amount": 6106,
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
    "name": "Liam Smith",
    "email": {
      "value": "riley.2932@example.com"
    },
    "id": "cust_cf0d3d99ef8b4028b5eec081",
    "phone_number": "+18356122796"
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
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "7420 Pine Blvd"
      },
      "line2": {
        "value": "9997 Pine Ln"
      },
      "line3": {
        "value": "3541 Lake Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "33641"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "alex.8297@sandbox.example.com"
      },
      "phone_number": {
        "value": "1190026354"
      },
      "phone_country_code": "+32"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2155 Market St"
      },
      "line2": {
        "value": "3076 Lake St"
      },
      "line3": {
        "value": "4645 Pine Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "98066"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "sam.5443@example.com"
      },
      "phone_number": {
        "value": "9385076488"
      },
      "phone_country_code": "+32"
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
  "description": "No3DS auto capture Bancontact payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bancontact_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:11:23 GMT
x-request-id: authorize_no3ds_auto_capture_bancontact_req

Response contents:
{
  "merchantTransactionId": "mti_c6a6b211018c4c2eb1cc0f2a",
  "connectorTransactionId": "46054246",
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
    "cf-ray": "9e13849cf8ba57ad-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 06:11:23 GMT",
    "hour-limit-remaining": "3000",
    "minute-limit-remaining": "95",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=lTKaZKTu4D0IgmAJWA.j_sHCGZ4W5F_x18RlFs4joLs-1774332681.757146-1.0.1.1-Q7.WWfZveNcoJRMpXJHfi31FNJ_g7LeebaticyQ5oSC3iH_E2nrFCINrK5PEyTD0oCrwDWam5KI3Vunxi2mCwMoiRE7wjNUx4i5q2_9Bod6PXasecdNyuyFAvJmNM_mx382YOfk4yOqcfNXNbvtFOQ; HttpOnly; Secure; Path=/; Domain=helcim.com; Expires=Tue, 24 Mar 2026 06:41:23 GMT",
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
