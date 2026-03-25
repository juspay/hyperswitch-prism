# Connector `bluesnap` / Suite `authorize` / Scenario `no3ds_auto_capture_debit_card`

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
  "merchant_transaction_id": "mti_c89560466aa746bc8220e587",
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
        "value": "Emma Wilson"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "riley.9865@sandbox.example.com"
    },
    "id": "cust_19bd46a79f75488891477afb",
    "phone_number": "+447630785965"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "2244 Market St"
      },
      "line2": {
        "value": "5781 Lake Ln"
      },
      "line3": {
        "value": "1528 Sunset Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96080"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8954@sandbox.example.com"
      },
      "phone_number": {
        "value": "4761572269"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "1108 Oak Blvd"
      },
      "line2": {
        "value": "184 Market Ln"
      },
      "line3": {
        "value": "1422 Main Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59236"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.1246@sandbox.example.com"
      },
      "phone_number": {
        "value": "9248119420"
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
date: Mon, 23 Mar 2026 18:32:06 GMT
x-request-id: authorize_no3ds_auto_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "1087579054",
  "connectorTransactionId": "1087579054",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f84401da95644-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:32:05 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=wiheNqFwEWVg..JSabZvOXTbVQRKQ3J2UM1jQN4EVIY-1774290725-1.0.1.1-Ng3Lfi8p1Mkt6RH5YJMAsBGlQ_LdGpct5g2PsyU_OJbH6XxNa4b.cACOKL1cRFkeWqmvTW4ZkmS05fkrlh93QpTFnSaAlnPh2pEb9Fnw7rE; path=/; expires=Mon, 23-Mar-26 19:02:05 GMT; domain=.bluesnap.com; HttpOnly; Secure",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding"
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
