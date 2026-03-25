# Connector `cybersource` / Suite `authorize` / Scenario `no3ds_auto_capture_debit_card`

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
  "merchant_transaction_id": "mti_ae2b047ec9dd482fbb38a34c",
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
        "value": "Mia Taylor"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Johnson",
    "email": {
      "value": "casey.6113@testmail.io"
    },
    "id": "cust_ddc76184e11147b7859c88fb",
    "phone_number": "+913593789420"
  },
  "metadata": {
    "value": "{}"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9591 Sunset Blvd"
      },
      "line2": {
        "value": "362 Oak St"
      },
      "line3": {
        "value": "1844 Sunset Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97180"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.7179@testmail.io"
      },
      "phone_number": {
        "value": "2471075295"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "290 Sunset Ave"
      },
      "line2": {
        "value": "4955 Sunset St"
      },
      "line3": {
        "value": "9964 Oak Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "44713"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1818@example.com"
      },
      "phone_number": {
        "value": "1817261774"
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:40:53 GMT
x-request-id: authorize_no3ds_auto_capture_debit_card_req

Response contents:
{
  "merchantTransactionId": "mti_ae2b047ec9dd482fbb38a34c",
  "connectorTransactionId": "7742912529956215804807",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, must-revalidate",
    "content-length": "1728",
    "content-type": "application/hal+json",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "91ff4270-4bbe-466c-a22d-2ab5b4e52174",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-22556320",
    "x-requestid": "7742912529956215804807",
    "x-response-time": "442ms"
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
