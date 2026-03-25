# Connector `xendit` / Suite `authorize` / Scenario `threeds_manual_capture_credit_card`

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
  -H "x-request-id: authorize_threeds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_d534e49cf4a246ce9d487f1a",
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
        "value": "Ethan Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "casey.1438@sandbox.example.com"
    },
    "id": "cust_36469e0adce04d22884f00bc",
    "phone_number": "+11500937979"
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
        "value": "Wilson"
      },
      "line1": {
        "value": "7902 Market Blvd"
      },
      "line2": {
        "value": "1579 Pine Dr"
      },
      "line3": {
        "value": "2318 Lake Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "49181"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "alex.7256@sandbox.example.com"
      },
      "phone_number": {
        "value": "2119913038"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "2346 Market St"
      },
      "line2": {
        "value": "3642 Sunset Blvd"
      },
      "line3": {
        "value": "449 Market Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "68257"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.9118@sandbox.example.com"
      },
      "phone_number": {
        "value": "6663970846"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "THREE_DS",
  "enrolled_for_3ds": true,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "3DS manual capture card payment (credit)",
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
x-connector-request-reference-id: authorize_threeds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_threeds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:24 GMT
x-request-id: authorize_threeds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "67aaccf2-3636-43f8-bc60-1ef430ccd38f",
  "connectorTransactionId": "pr-759f9a84-c8ca-4fa7-a4ff-748182080506",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e135156fe66054c-BOM",
    "connection": "keep-alive",
    "content-length": "1950",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:23 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "54",
    "rate-limit-reset": "44.782",
    "request-id": "69c222d5000000001d198f43752ba18a",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=NDeNxFgGnwuOz0RA.RGJvSBQhyJN0D8NKBvImtvwBaU-1774330581.5931659-1.0.1.1-.5U5h.Qih9iyD2doLWjZeZmKABHUdn9dqKuYXdY8pUdFMPPTKTYEND2H8rff2FtHDYaXbOxsNLxSyNTdQBpa0XzdOAOxTv4aRxd7makeHVCilzaVtMPotjv6T4VhCEJ2; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:23 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1637"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://redirect.xendit.co/authentications/69c222d730bf514d3441531f/render?api_key=xnd_public_development_wv35H4ptnhygdbzZacS2TGSkm9vy3BBy_UbEmOHIz3CRHzAhinlyOzelErBnkQ7R",
      "method": "HTTP_METHOD_GET"
    }
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
