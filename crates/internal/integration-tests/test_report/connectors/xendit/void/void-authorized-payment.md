# Connector `xendit` / Suite `void` / Scenario `void_authorized_payment`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk HTTP request failed for 'void'/'void_authorized_payment': builder error
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — FAIL</summary>

**Dependency Error**

```text
sdk call failed: sdk HTTP request failed for 'create_customer'/'create_customer': builder error
```

<details>
<summary>Show Dependency Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

_Response trace not available._

</details>

</details>
<details>
<summary>2. authorize(no3ds_manual_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_9b682284a0dc4afeb76ce383e7385424",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": "***MASKED***",
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": "***MASKED***",
      "card_holder_name": {
        "value": "Ethan Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "jordan.3532@sandbox.example.com"
    },
    "id": "cust_f0b0b1faf6004d1bbd59171fc7b7f136",
    "phone_number": "+443899333208"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "9743 Market Blvd"
      },
      "line2": {
        "value": "8193 Market Blvd"
      },
      "line3": {
        "value": "168 Lake Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "59605"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5800@example.com"
      },
      "phone_number": {
        "value": "3031512166"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "1710 Main Ln"
      },
      "line2": {
        "value": "2449 Main Dr"
      },
      "line3": {
        "value": "4229 Main Ln"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "54728"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5784@sandbox.example.com"
      },
      "phone_number": {
        "value": "7349236032"
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
  "description": "No3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
}
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```json
{
  "status": "ATTEMPT_STATUS_UNSPECIFIED",
  "error": {
    "issuer_details": {
      "network_details": {}
    },
    "connector_details": {
      "code": "API_VALIDATION_ERROR",
      "message": "Country 'US' is currently not supported"
    }
  },
  "status_code": 400,
  "response_headers": {
    "rate-limit-limit": "60",
    "content-length": "90",
    "date": "Mon, 23 Mar 2026 12:33:40 GMT",
    "rate-limit-reset": "50.388",
    "cf-cache-status": "DYNAMIC",
    "content-type": "application/json",
    "connection": "keep-alive",
    "set-cookie": "__cf_bm=pMdNGmm0KhZn7a__JEmLoRARMoo2di_6sDo1L26d830-1774269220.447195-1.0.1.1-D7qd0GnrMisBzB5R.nGijyASuiFWJbVgxQiPyG9o99OVFDSWjsEgDOSz5Xu1uzz0Hy2.lN9s0qEGLRMDhIXnMs_ULmVIyc1S.zOZ9yRFm7ByaDzqsNCnBL81zk2Va6g5; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Mon, 23 Mar 2026 13:03:40 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "223",
    "rate-limit-remaining": "46",
    "cf-ray": "9e0d7743c9890b2c-BOM",
    "request-id": "69c13324000000003c440aacf87d8986",
    "server": "cloudflare",
    "access-control-allow-origin": "*"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
