# Connector `noon` / Suite `capture` / Scenario `capture_full_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
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
  "merchant_transaction_id": "mti_e7c41c328750489fbb3368528155cf41",
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
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Brown",
    "email": {
      "value": "casey.4236@example.com"
    },
    "id": "cust_612f52dbc9274ba093a233a92d672860",
    "phone_number": "+913195343139"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "527 Lake Rd"
      },
      "line2": {
        "value": "4953 Sunset Rd"
      },
      "line3": {
        "value": "3372 Market Rd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "96072"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4851@example.com"
      },
      "phone_number": {
        "value": "1525038135"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4817 Lake Dr"
      },
      "line2": {
        "value": "3881 Market St"
      },
      "line3": {
        "value": "3308 Main Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "25085"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.8112@example.com"
      },
      "phone_number": {
        "value": "3687247687"
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
      "code": "1578",
      "message": "IP restriction applied, location not allowed.",
      "reason": "IP restriction applied, location not allowed."
    }
  },
  "status_code": 401,
  "response_headers": {
    "x-merchantid": "hyperswitch",
    "x-message": "IP restriction applied, location not allowed.",
    "alt-svc": "h3=\":443\"; ma=93600",
    "x-resultcode": "1578",
    "x-apioperation": "INITIATE",
    "akamai-cache-status": "Miss from child, Miss from parent",
    "content-type": "application/json; charset=utf-8",
    "x-classdescription": "Rejected AccessDenied",
    "referrer-policy": "no-referrer-when-downgrade",
    "content-length": "230",
    "np-waf-trace-id": "0.ad277368.1774268281.109b4cb8",
    "x-content-type-options": "nosniff",
    "connection": "close",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "cache-control": "max-age=0",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "date": "Mon, 23 Mar 2026 12:18:01 GMT",
    "server-timing": "ak_p; desc=\"1774268280979_1752377261_278613176_26867_6969_8_20_-\";dur=1"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```json
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_40e456360c1a40e490b930bba84c4f87"
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "status": "ATTEMPT_STATUS_UNSPECIFIED",
  "error": {
    "connector_details": {
      "code": "1578",
      "message": "IP restriction applied, location not allowed.",
      "reason": "IP restriction applied, location not allowed."
    }
  },
  "status_code": 401,
  "response_headers": {
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-content-type-options": "nosniff",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "content-type": "application/json; charset=utf-8",
    "server-timing": "ak_p; desc=\"1774268281315_1752377239_157572488_26782_6532_9_16_-\";dur=1",
    "connection": "close",
    "x-merchantid": "hyperswitch",
    "referrer-policy": "no-referrer-when-downgrade",
    "x-classdescription": "Rejected AccessDenied",
    "date": "Mon, 23 Mar 2026 12:18:01 GMT",
    "np-waf-trace-id": "0.97277368.1774268281.9645d88",
    "content-length": "230",
    "x-message": "IP restriction applied, location not allowed.",
    "akamai-cache-status": "Miss from child, Miss from parent",
    "x-apioperation": "CAPTURE",
    "x-resultcode": "1578",
    "alt-svc": "h3=\":443\"; ma=93600",
    "cache-control": "max-age=0"
  }
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
