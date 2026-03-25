# Connector `checkout` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

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
  "merchant_recurring_payment_id": "mrpi_fe2b7130dc734bdf92252984608688b9",
  "amount": {
    "minor_amount": 4500,
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
        "value": "Mia Miller"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "morgan.2611@sandbox.example.com"
    },
    "id": "cust_27bd0608908b47539e0c13e5a78a7ec8",
    "phone_number": "+444815649658"
  },
  "webhook_url": "https://example.com/payment/webhook",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4465 Lake St"
      },
      "line2": {
        "value": "9541 Main Blvd"
      },
      "line3": {
        "value": "8239 Lake Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "77190"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.3234@example.com"
      },
      "phone_number": {
        "value": "5387725725"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION",
  "return_url": "https://example.com/payment/return"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_recurring_payment_id": "pay_etkwmkyb5uxyffyjflcdyskswu",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "location": "https://api.sandbox.checkout.com/payments/pay_etkwmkyb5uxyffyjflcdyskswu",
    "connection": "keep-alive",
    "date": "Mon, 23 Mar 2026 12:11:01 GMT",
    "content-length": "1873",
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;",
    "content-type": "application/json; charset=utf-8",
    "cko-request-id": "b0e4f461-d319-462e-af65-79bc6a74e0a2",
    "cko-version": "1.1677.0+d6ddd2b"
  },
  "mandate_reference": {
    "mandate_id_type": {
      "ConnectorMandateId": {
        "connector_mandate_id": "src_ova7jblq46aennzyyvmfg5q7au",
        "connector_mandate_request_reference_id": "pay_etkwmkyb5uxyffyjflcdyskswu"
      }
    }
  },
  "network_transaction_id": "967980748330804",
  "merchant_recurring_payment_id": "mrpi_fe2b7130dc734bdf92252984608688b9",
  "captured_amount": 4500,
  "connector_feature_data": "{\"psync_flow\":\"Capture\"}"
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
