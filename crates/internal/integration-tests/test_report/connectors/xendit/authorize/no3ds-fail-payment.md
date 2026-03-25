# Connector `xendit` / Suite `authorize` / Scenario `no3ds_fail_payment`

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
  "merchant_transaction_id": "mti_3234c0e0497d42e6a6e43347",
  "amount": {
    "minor_amount": 100,
    "currency": "IDR"
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
        "value": "Ava Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "jordan.3034@example.com"
    },
    "id": "cust_4f69af06ff5045949d832145",
    "phone_number": "+13181342336"
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
        "value": "Miller"
      },
      "line1": {
        "value": "7292 Sunset Ave"
      },
      "line2": {
        "value": "7320 Market Ave"
      },
      "line3": {
        "value": "800 Oak Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83558"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "morgan.4957@example.com"
      },
      "phone_number": {
        "value": "7682518221"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8415 Market Rd"
      },
      "line2": {
        "value": "4113 Market St"
      },
      "line3": {
        "value": "2166 Oak Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "19476"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.3448@testmail.io"
      },
      "phone_number": {
        "value": "9636957922"
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
date: Tue, 24 Mar 2026 05:36:14 GMT
x-request-id: authorize_no3ds_fail_payment_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "API_VALIDATION_ERROR",
      "message": "Amount below minimum limit"
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e1351204894054c-BOM",
    "connection": "keep-alive",
    "content-length": "77",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:14 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "57",
    "rate-limit-reset": "53.526",
    "request-id": "69c222cc00000000231c47ab9e014c5a",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=1RyMG5F7wfSn4ht4OUKSKtLZvBF8jYLaoaxW3sla.q4-1774330572.845957-1.0.1.1-AK0xN5EpbLCIUVIN_hqV5_eOR_DOjY_zRU.mRLPvhIum.ZZRY9RboeZOJSA1z5kkmRLmL9z9IWZemrUqdcLCYh69OLT1FDwOJNspY0P1Hajf1lcQJKiZlQRV_.aRTMc.; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:14 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1310"
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
