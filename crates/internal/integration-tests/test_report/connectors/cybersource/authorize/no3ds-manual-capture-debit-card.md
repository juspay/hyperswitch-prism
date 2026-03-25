# Connector `cybersource` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

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
  "merchant_transaction_id": "mti_bba04063a7cf4f31985e0233",
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
        "value": "Noah Johnson"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Miller",
    "email": {
      "value": "riley.1729@sandbox.example.com"
    },
    "id": "cust_101db242c95f4ccda353f5c4",
    "phone_number": "+15487782633"
  },
  "metadata": {
    "value": "{}"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "3025 Sunset Ave"
      },
      "line2": {
        "value": "2587 Oak St"
      },
      "line3": {
        "value": "7535 Main Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "72409"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.5242@testmail.io"
      },
      "phone_number": {
        "value": "8093624366"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3967 Market Ave"
      },
      "line2": {
        "value": "4673 Sunset Ln"
      },
      "line3": {
        "value": "4591 Oak Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "20259"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5822@testmail.io"
      },
      "phone_number": {
        "value": "3955229435"
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
  "connector_feature_data": {
    "value": "{\"disable_avs\":false,\"disable_cvn\":false}"
  }
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
date: Mon, 23 Mar 2026 18:40:56 GMT
x-request-id: authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "mti_bba04063a7cf4f31985e0233",
  "connectorTransactionId": "7742912562266533004805",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-length": "1848",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "81f5cfe6-7ef4-4c0b-9703-c4749f32ba1d",
    "x-opnet-transaction-trace": "c6da0384-aff8-4d30-af35-f1073880e050-2349521-23754326",
    "x-requestid": "7742912562266533004805",
    "x-response-time": "291ms"
  },
  "networkTransactionId": "016150703802094",
  "incrementalAuthorizationAllowed": ***MASKED***
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzcG9uc2UiOnsiY29kZSI6IlkiLCJjb2RlUmF3IjoiWSJ9LCJjYXJkX3ZlcmlmaWNhdGlvbiI6bnVsbH0="
      }
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
