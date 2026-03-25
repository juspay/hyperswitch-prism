# Connector `wellsfargo` / Suite `setup_recurring` / Scenario `setup_recurring`

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
  "merchant_recurring_payment_id": "mrpi_6aa7ebbe886a469b963d0b11484aaeb7",
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
        "value": "Liam Miller"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "casey.6228@sandbox.example.com"
    },
    "id": "cust_786451ab921c4a8f9ac719f019bd673a",
    "phone_number": "+17316220074"
  },
  "setup_future_usage": "OFF_SESSION",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5155 Sunset Rd"
      },
      "line2": {
        "value": "1472 Main Ln"
      },
      "line3": {
        "value": "6246 Sunset Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "71294"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4123@testmail.io"
      },
      "phone_number": {
        "value": "5306506153"
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
  "connector_recurring_payment_id": "7742691104626883204807",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "content-length": "1504",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-response-time": "208ms",
    "content-type": "application/hal+json",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "v-c-correlation-id": "cf1ea93c-eca2-4d8e-a6a0-01fb00db3771",
    "expires": "-1",
    "x-opnet-transaction-trace": "d0b8bcd4-86a9-4402-95e8-35c26c084b92-2277564-19158871",
    "x-requestid": "7742691104626883204807"
  },
  "mandate_reference": {
    "mandate_id_type": {
      "ConnectorMandateId": {
        "connector_mandate_id": "7742691104626883204807"
      }
    }
  },
  "network_transaction_id": "016150703802094",
  "merchant_recurring_payment_id": "mrpi_6aa7ebbe886a469b963d0b11484aaeb7",
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
  "captured_amount": 6000
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
