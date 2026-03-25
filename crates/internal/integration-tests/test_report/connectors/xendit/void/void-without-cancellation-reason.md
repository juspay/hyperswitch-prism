# Connector `xendit` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk HTTP request failed for 'void'/'void_without_cancellation_reason': builder error
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
<summary>2. authorize(no3ds_manual_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

<details>
<summary>Show Dependency Request (masked)</summary>

```json
{
  "merchant_transaction_id": "mti_fcd40e3f6f9344f2bbd85b80d32d8726",
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
        "value": "Ethan Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Wilson",
    "email": {
      "value": "casey.1491@example.com"
    },
    "id": "cust_6554829d1d3d47a5a5473cf09573c535",
    "phone_number": "+17567624110"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "8550 Main Ave"
      },
      "line2": {
        "value": "6789 Lake St"
      },
      "line3": {
        "value": "9383 Main Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "20638"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2845@example.com"
      },
      "phone_number": {
        "value": "5890632293"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "375 Oak Ln"
      },
      "line2": {
        "value": "3537 Market Ln"
      },
      "line3": {
        "value": "4731 Lake Blvd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "79880"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.1456@sandbox.example.com"
      },
      "phone_number": {
        "value": "7994009697"
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
  "description": "No3DS manual capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true
}
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```json
{
  "status": "ATTEMPT_STATUS_UNSPECIFIED",
  "error": {
    "issuer_details": {
      "network_details": {}
    },
    "connector_details": {
      "code": "API_VALIDATION_ERROR",
      "message": "Country 'US' is currently not supported"
    }
  },
  "status_code": 400,
  "response_headers": {
    "rate-limit-reset": "49.574",
    "set-cookie": "__cf_bm=nmpGtldMPhqGnUntOqECahSaP3P03NisP.tObVVa_.g-1774269221.275126-1.0.1.1-2QRinp_6A0DHnX9EihlBvOOznk4YMU18nUGhL38K47y4KOuFEDJSzqJPERh0j37s8UKva9zOlNGTchIkywciIe0LAlk2afPi.7zMvEVXKVQvS5zEMlhArFkgVwXOptSL; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Mon, 23 Mar 2026 13:03:41 GMT",
    "server": "cloudflare",
    "cf-cache-status": "DYNAMIC",
    "request-id": "69c1332500000000203bbb4cb6685a80",
    "content-length": "90",
    "date": "Mon, 23 Mar 2026 12:33:41 GMT",
    "content-type": "application/json",
    "vary": "Origin",
    "connection": "keep-alive",
    "rate-limit-limit": "60",
    "access-control-allow-origin": "*",
    "rate-limit-remaining": "44",
    "x-envoy-upstream-service-time": "173",
    "cf-ray": "9e0d7748fb5eff64-BOM"
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


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
