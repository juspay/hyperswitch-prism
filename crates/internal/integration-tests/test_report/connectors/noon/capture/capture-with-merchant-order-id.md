# Connector `noon` / Suite `capture` / Scenario `capture_with_merchant_order_id`

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
  "merchant_transaction_id": "mti_efde6fc34253490cbbe612bfd030921c",
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
        "value": "Noah Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "morgan.4145@sandbox.example.com"
    },
    "id": "cust_b0ca71e7a4784b37b10077800cf91fa9",
    "phone_number": "+18928597234"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4732 Market Blvd"
      },
      "line2": {
        "value": "6880 Oak Dr"
      },
      "line3": {
        "value": "3724 Market Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "39328"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.5672@testmail.io"
      },
      "phone_number": {
        "value": "1023101106"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "3479 Pine Rd"
      },
      "line2": {
        "value": "130 Sunset St"
      },
      "line3": {
        "value": "6347 Lake Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "55871"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8771@sandbox.example.com"
      },
      "phone_number": {
        "value": "7913577276"
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
    "np-waf-trace-id": "0.97277368.1774268282.9645df5",
    "connection": "close",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "x-classdescription": "Rejected AccessDenied",
    "server-timing": "ak_p; desc=\"1774268282350_1752377239_157572597_26293_6165_12_20_-\";dur=1",
    "referrer-policy": "no-referrer-when-downgrade",
    "x-merchantid": "hyperswitch",
    "content-type": "application/json; charset=utf-8",
    "cache-control": "max-age=0",
    "date": "Mon, 23 Mar 2026 12:18:02 GMT",
    "x-resultcode": "1578",
    "alt-svc": "h3=\":443\"; ma=93600",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-content-type-options": "nosniff",
    "akamai-cache-status": "Miss from child, Miss from parent",
    "x-apioperation": "INITIATE",
    "content-length": "230",
    "x-message": "IP restriction applied, location not allowed."
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
  "merchant_capture_id": "mci_dfd50ae6f5c34b2a8e56138aa090da1e",
  "merchant_order_id": "gen_519857"
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
    "cache-control": "max-age=0",
    "date": "Mon, 23 Mar 2026 12:18:02 GMT",
    "content-length": "230",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "akamai-cache-status": "Miss from child, Miss from parent",
    "np-waf-trace-id": "0.ad277368.1774268282.109b4f6c",
    "server-timing": "ak_p; desc=\"1774268282691_1752377261_278613868_24754_6355_8_20_-\";dur=1",
    "referrer-policy": "no-referrer-when-downgrade",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-resultcode": "1578",
    "x-content-type-options": "nosniff",
    "x-classdescription": "Rejected AccessDenied",
    "connection": "close",
    "content-type": "application/json; charset=utf-8",
    "x-apioperation": "CAPTURE",
    "x-merchantid": "hyperswitch",
    "x-message": "IP restriction applied, location not allowed.",
    "alt-svc": "h3=\":443\"; ma=93600"
  }
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
