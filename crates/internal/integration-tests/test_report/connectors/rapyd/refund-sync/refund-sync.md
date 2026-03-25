# Connector `rapyd` / Suite `refund_sync` / Scenario `refund_sync`

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
  "merchant_transaction_id": "mti_68b7b206b36f4b2190bed43c235ed6d7",
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
        "value": "Mia Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Brown",
    "email": {
      "value": "riley.8099@example.com"
    },
    "id": "cust_67efe37873294ebda00599bd04db8cfe",
    "phone_number": "+447077087310"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "9329 Lake Dr"
      },
      "line2": {
        "value": "8812 Lake St"
      },
      "line3": {
        "value": "899 Sunset St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "56340"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8799@example.com"
      },
      "phone_number": {
        "value": "7073918483"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "1786 Oak Dr"
      },
      "line2": {
        "value": "6633 Sunset Ave"
      },
      "line3": {
        "value": "9267 Sunset Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "79070"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4949@testmail.io"
      },
      "phone_number": {
        "value": "7787219917"
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
  "merchant_transaction_id": "mti_68b7b206b36f4b2190bed43c235ed6d7",
  "connector_transaction_id": "payment_59534f52974d49f30e198642fa9d5dc1",
  "status": "CHARGED",
  "status_code": 200,
  "response_headers": {
    "access-control-allow-origin": "*",
    "strict-transport-security": "max-age=8640000; includeSubDomains",
    "content-length": "2313",
    "cf-ray": "9e0d64563ebb7226-MRS",
    "server": "cloudflare",
    "connection": "keep-alive",
    "date": "Mon, 23 Mar 2026 12:20:46 GMT",
    "cf-cache-status": "DYNAMIC",
    "content-type": "application/json; charset=utf-8",
    "set-cookie": "_cfuvid=e4Asf901DSs0at.LHxABrZGX5nsKsIZ_EsMCEf3OzhA-1774268445.1606846-1.0.1.1-2Vs7rFQEVoHaARMUHHXr_w0Jmr01_n1AhwSC6PEmPB0; HttpOnly; SameSite=None; Secure; Path=/; Domain=rapyd.net",
    "etag": "W/\"909-9fG7/U/DiMOWUTrcgjmQjZ7y7ZY\""
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
