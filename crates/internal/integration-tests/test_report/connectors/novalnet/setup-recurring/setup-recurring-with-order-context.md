# Connector `novalnet` / Suite `setup_recurring` / Scenario `setup_recurring_with_order_context`

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
  "merchant_recurring_payment_id": "mrpi_810f78d2e5c6421ab9401f90a4d0fd67",
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
        "value": "Mia Taylor"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Emma Miller",
    "email": {
      "value": "morgan.6014@example.com"
    },
    "id": "cust_a1da3af8e92e4698bfd5b637e25bcbe1",
    "phone_number": "+18409185591"
  },
  "complete_authorize_url": "https://example.com/payment/complete",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6219 Sunset Blvd"
      },
      "line2": {
        "value": "8329 Market Dr"
      },
      "line3": {
        "value": "8303 Sunset Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "46277"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1574@testmail.io"
      },
      "phone_number": {
        "value": "1564363246"
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
  "merchant_order_id": "gen_275222",
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
  "status": "AUTHENTICATION_PENDING",
  "status_code": 200,
  "response_headers": {
    "referrer-policy": "no-referrer",
    "access-control-allow-origin": "*",
    "x-frame-options": "SAMEORIGIN",
    "cross-origin-opener-policy": "same-origin",
    "content-type": "application/json",
    "x-content-type-options": "nosniff",
    "connection": "Upgrade",
    "x-download-options": "noopen",
    "upgrade": "h2,h2c",
    "cross-origin-resource-policy": "same-origin",
    "server": "Apache",
    "x-permitted-cross-domain-policies": "none",
    "permissions-policy": "accelerometer=(), camera=(), geolocation=(), gyroscope=(), magnetometer=(), microphone=(), payment=(), usb=()",
    "content-length": "287",
    "x-dns-prefetch-control": "off",
    "strict-transport-security": "max-age=15552000; includeSubDomains",
    "cross-origin-embedder-policy": "require-corp",
    "content-security-policy": "default-src 'self'; base-uri 'self'; font-src 'self' https: data:; form-action 'self'; frame-ancestors 'self'; img-src 'self' data:; object-src 'none'; script-src 'self'; script-src-attr 'none'; style-src 'self' https: 'unsafe-inline'; upgrade-insecure-requests",
    "date": "Mon, 23 Mar 2026 12:18:49 GMT",
    "x-xss-protection": "0",
    "origin-agent-cluster": "?1"
  },
  "redirection_data": {
    "form_type": {
      "Form": {
        "endpoint": "https://payport.novalnet.de/pci_payport/txn_secret/ba5ce38816df0dc191373a295a0e3d85",
        "method": "HTTP_METHOD_GET",
        "form_fields": {}
      }
    }
  }
}
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
