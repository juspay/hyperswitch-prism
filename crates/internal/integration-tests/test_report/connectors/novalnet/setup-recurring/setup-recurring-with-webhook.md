# Connector `novalnet` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

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
  "merchant_recurring_payment_id": "mrpi_5c4abdb53738466897ad4ec69a9a9aee",
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
        "value": "Emma Wilson"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Liam Taylor",
    "email": {
      "value": "riley.5133@testmail.io"
    },
    "id": "cust_3f3d71c19b294d7f8ef29b6170c699a8",
    "phone_number": "+17939539485"
  },
  "webhook_url": "https://example.com/payment/webhook",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "6160 Sunset Rd"
      },
      "line2": {
        "value": "4283 Pine Ave"
      },
      "line3": {
        "value": "4463 Lake Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "24069"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7527@testmail.io"
      },
      "phone_number": {
        "value": "9962970199"
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
  "status": "AUTHENTICATION_PENDING",
  "status_code": 200,
  "response_headers": {
    "origin-agent-cluster": "?1",
    "x-dns-prefetch-control": "off",
    "server": "Apache",
    "x-xss-protection": "0",
    "date": "Mon, 23 Mar 2026 12:18:50 GMT",
    "x-permitted-cross-domain-policies": "none",
    "content-length": "287",
    "connection": "Upgrade",
    "cross-origin-embedder-policy": "require-corp",
    "cross-origin-opener-policy": "same-origin",
    "x-frame-options": "SAMEORIGIN",
    "content-type": "application/json",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "x-download-options": "noopen",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "x-content-type-options": "nosniff",
    "access-control-allow-origin": "*",
    "upgrade": "h2,h2c",
    "referrer-policy": "no-referrer",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "cross-origin-resource-policy": "same-origin"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/b015e256057bda6b7619417a67af213b",
        "method": "HTTP_METHOD_GET",
        "form_fields": {}
      }
    }
  }
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
