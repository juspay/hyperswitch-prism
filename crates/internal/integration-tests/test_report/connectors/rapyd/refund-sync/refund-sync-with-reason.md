# Connector `rapyd` / Suite `refund_sync` / Scenario `refund_sync_with_reason`

- Service: `RefundService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
unsupported suite 'refund_sync' for grpcurl generation
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_1f4c81f7587f42df9357fe56a465264c",
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
        "value": "Liam Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "sam.8147@example.com"
    },
    "id": "cust_2677662c36ff418e90c8cda695265be7",
    "phone_number": "+11878855448"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1461 Pine Dr"
      },
      "line2": {
        "value": "4337 Pine Blvd"
      },
      "line3": {
        "value": "9203 Oak Blvd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "31764"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.8822@sandbox.example.com"
      },
      "phone_number": {
        "value": "9596179180"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "9780 Lake Ln"
      },
      "line2": {
        "value": "9370 Oak Dr"
      },
      "line3": {
        "value": "871 Oak St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35257"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.2892@testmail.io"
      },
      "phone_number": {
        "value": "9978498819"
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
  "test_mode": true
}
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_1f4c81f7587f42df9357fe56a465264c",
  "connector_transaction_id": "payment_42e7257883a34783740ba26b82179f4d",
  "status": "CHARGED",
  "status_code": 200,
  "response_headers": {
    "server": "cloudflare",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0d645ff9cab698-MRS",
    "etag": "W/\"908-podfC4Wz6Co46FQsm25MT8AmCrI\"",
    "date": "Mon, 23 Mar 2026 12:20:47 GMT",
    "set-cookie": "_cfuvid=ugxPR3P1XScL7KYUCRrA3fifIIbxR0cJj3CIQCKUZE4-1774268446.7186608-1.0.1.1-kLy1FbDE6W3FyR47no7kVsU35buyCG_H_DVzZvrvhpM; HttpOnly; SameSite=None; Secure; Path=/; Domain=rapyd.net",
    "content-type": "application/json; charset=utf-8",
    "content-length": "2312",
    "connection": "keep-alive",
    "access-control-allow-origin": "*",
    "strict-transport-security": "max-age=8640000; includeSubDomains"
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
