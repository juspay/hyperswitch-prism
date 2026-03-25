# Connector `wellsfargo` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

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
  "merchant_recurring_payment_id": "mrpi_d54e1a7eb045436aa16812519de06aed",
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
        "value": "Noah Johnson"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "alex.1398@example.com"
    },
    "id": "cust_a9c2414faf494d58b83dccdc200cd955",
    "phone_number": "+14435750885"
  },
  "webhook_url": "https://example.com/payment/webhook",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3116 Main Ln"
      },
      "line2": {
        "value": "4224 Main Blvd"
      },
      "line3": {
        "value": "7586 Sunset Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "65718"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.3555@testmail.io"
      },
      "phone_number": {
        "value": "5372364799"
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
  "connector_recurring_payment_id": "7742691126876579404806",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "strict-transport-security": "max-age=31536000",
    "x-requestid": "7742691126876579404806",
    "v-c-correlation-id": "62a9822a-14a9-4738-8905-4828aae563a2",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19813854",
    "expires": "-1",
    "content-type": "application/hal+json",
    "x-response-time": "216ms",
    "content-length": "1504",
    "pragma": "no-cache"
  },
  "mandate_reference": {
    "mandate_id_type": {
      "ConnectorMandateId": {
        "connector_mandate_id": "7742691126876579404806"
      }
    }
  },
  "network_transaction_id": "016150703802094",
  "merchant_recurring_payment_id": "mrpi_d54e1a7eb045436aa16812519de06aed",
  "connector_response": {
    "additional_payment_method_data": {
      "payment_method_data": {
        "Card": {
          "payment_checks": [
            123,
            34,
            97,
            118,
            115,
            95,
            114,
            101,
            115,
            112,
            111,
            110,
            115,
            101,
            34,
            58,
            123,
            34,
            99,
            111,
            100,
            101,
            34,
            58,
            34,
            89,
            34,
            44,
            34,
            99,
            111,
            100,
            101,
            82,
            97,
            119,
            34,
            58,
            34,
            89,
            34,
            125,
            44,
            34,
            99,
            97,
            114,
            100,
            95,
            118,
            101,
            114,
            105,
            102,
            105,
            99,
            97,
            116,
            105,
            111,
            110,
            34,
            58,
            110,
            117,
            108,
            108,
            125
          ]
        }
      }
    }
  },
  "incremental_authorization_allowed": "***MASKED***",
  "captured_amount": 4500
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
