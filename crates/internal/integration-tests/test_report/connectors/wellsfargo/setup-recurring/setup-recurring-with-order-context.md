# Connector `wellsfargo` / Suite `setup_recurring` / Scenario `setup_recurring_with_order_context`

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
  "merchant_recurring_payment_id": "mrpi_88f425adaf3e4b79ba95da7e806946b5",
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
    "name": "Liam Brown",
    "email": {
      "value": "riley.7656@example.com"
    },
    "id": "cust_59822a74135742a589a2ef789307e42c",
    "phone_number": "+448748234423"
  },
  "complete_authorize_url": "https://example.com/payment/complete",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "9034 Lake Dr"
      },
      "line2": {
        "value": "6971 Market Dr"
      },
      "line3": {
        "value": "3293 Market Ave"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "34530"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.6657@testmail.io"
      },
      "phone_number": {
        "value": "8233795500"
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
  "off_session": true,
  "merchant_order_id": "gen_286605",
  "order_category": "subscription",
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "connector_recurring_payment_id": "7742691115916579004806",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "content-length": "1504",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000",
    "content-type": "application/hal+json",
    "x-response-time": "211ms",
    "expires": "-1",
    "cache-control": "no-cache, no-store, must-revalidate",
    "x-requestid": "7742691115916579004806",
    "x-opnet-transaction-trace": "687b1be9-a770-4c1b-984d-e09c8a4b9f43-2283790-19813704",
    "v-c-correlation-id": "2972b28e-e0e8-4ed4-a50f-7e5bfcba1234"
  },
  "mandate_reference": {
    "mandate_id_type": {
      "ConnectorMandateId": {
        "connector_mandate_id": "7742691115916579004806"
      }
    }
  },
  "network_transaction_id": "016150703802094",
  "merchant_recurring_payment_id": "mrpi_88f425adaf3e4b79ba95da7e806946b5",
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
