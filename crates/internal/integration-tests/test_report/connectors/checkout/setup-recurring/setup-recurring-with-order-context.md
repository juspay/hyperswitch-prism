# Connector `checkout` / Suite `setup_recurring` / Scenario `setup_recurring_with_order_context`

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
  "merchant_recurring_payment_id": "mrpi_95351a3e1df64f81b6de0927031f8f94",
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
      "value": "alex.7599@example.com"
    },
    "id": "cust_2e93c03409dc448cae1d4b99f4ffdaca",
    "phone_number": "+912596879947"
  },
  "complete_authorize_url": "https://example.com/payment/complete",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "4108 Oak Dr"
      },
      "line2": {
        "value": "3250 Lake St"
      },
      "line3": {
        "value": "8797 Main St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55409"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2094@example.com"
      },
      "phone_number": {
        "value": "1131871633"
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
  "merchant_order_id": "gen_620400",
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
  "connector_recurring_payment_id": "pay_ma43tkqbdrhi3gnvjlc47u5swm",
  "status": "CHARGED",
  "status_code": 201,
  "response_headers": {
    "strict-transport-security": "max-age=16000000; includeSubDomains; preload;",
    "cko-request-id": "420c9212-2d96-94ed-9a6c-0903a138d5a0",
    "content-type": "application/json; charset=utf-8",
    "cko-version": "1.1677.0+d6ddd2b",
    "date": "Mon, 23 Mar 2026 12:11:00 GMT",
    "content-length": "1870",
    "connection": "keep-alive",
    "location": "https://api.sandbox.checkout.com/payments/pay_ma43tkqbdrhi3gnvjlc47u5swm"
  },
  "mandate_reference": {
    "mandate_id_type": {
      "ConnectorMandateId": {
        "connector_mandate_id": "src_gwt5xccxiv4e7id6nhysjdxt7a",
        "connector_mandate_request_reference_id": "pay_ma43tkqbdrhi3gnvjlc47u5swm"
      }
    }
  },
  "network_transaction_id": "770201112912286",
  "merchant_recurring_payment_id": "mrpi_95351a3e1df64f81b6de0927031f8f94",
  "captured_amount": 6000,
  "connector_feature_data": "{\"psync_flow\":\"Capture\"}"
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
