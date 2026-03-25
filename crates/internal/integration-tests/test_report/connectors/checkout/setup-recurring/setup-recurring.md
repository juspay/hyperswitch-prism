# Connector `checkout` / Suite `setup_recurring` / Scenario `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'mandate_reference.connector_mandate_id.connector_mandate_id': expected field to exist
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
<summary>Show Request (masked)</summary>

```json
{
  "merchant_recurring_payment_id": "mrpi_ea20fb7da18a432d9cd72203d3d81671",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
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
        "value": "Ethan Wilson"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Noah Miller",
    "email": {
      "value": "riley.1595@testmail.io"
    },
    "id": "cust_a900283048e146e1975283f86c647ccc",
    "phone_number": "+443493300253"
  },
  "setup_future_usage": "OFF_SESSION",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "649 Oak Rd"
      },
      "line2": {
        "value": "124 Main Dr"
      },
      "line3": {
        "value": "3290 Lake St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "97863"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4899@testmail.io"
      },
      "phone_number": {
        "value": "1568708944"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  }
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_recurring_payment_id": "pay_rtmsfyiblkoyxiu5j7m2ddvfyi",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 12:10:59 GMT",
    "cko-version": "1.1677.0+d6ddd2b",
    "location": "https://api.sandbox.checkout.com/payments/pay_rtmsfyiblkoyxiu5j7m2ddvfyi",
    "content-length": "1867",
    "connection": "keep-alive",
    "cko-request-id": "327a8be6-9e32-4304-a56e-52448e0b504b",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;"
  },
  "mandate_reference": {
    "mandate_id_type": {
      "ConnectorMandateId": {
        "connector_mandate_id": "src_7io7ptbu3kjudj2sx5ydn67eyy",
        "connector_mandate_request_reference_id": "pay_rtmsfyiblkoyxiu5j7m2ddvfyi"
      }
    }
  },
  "network_transaction_id": "118489097703020",
  "merchant_recurring_payment_id": "mrpi_ea20fb7da18a432d9cd72203d3d81671",
  "captured_amount": 6000,
  "connector_feature_data": "{\"psync_flow\":\"Capture\"}"
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
