# Connector `noon` / Suite `capture` / Scenario `capture_partial_amount`

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
  "merchant_transaction_id": "mti_7f554cc7486f49468574729d9467fe06",
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
        "value": "Ava Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Smith",
    "email": {
      "value": "morgan.4536@example.com"
    },
    "id": "cust_01e2b6fd8cc3419ca665cb53da635799",
    "phone_number": "+16301929149"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "7994 Main Blvd"
      },
      "line2": {
        "value": "7161 Pine Blvd"
      },
      "line3": {
        "value": "8881 Oak St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "16612"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4192@example.com"
      },
      "phone_number": {
        "value": "7439517104"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "6695 Sunset Blvd"
      },
      "line2": {
        "value": "7320 Main Rd"
      },
      "line3": {
        "value": "8246 Market Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "80373"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5207@testmail.io"
      },
      "phone_number": {
        "value": "3659999503"
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
    "akamai-cache-status": "Miss from child, Miss from parent",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "x-classdescription": "Rejected AccessDenied",
    "np-waf-trace-id": "0.ad277368.1774268281.109b4dd9",
    "referrer-policy": "no-referrer-when-downgrade",
    "connection": "close",
    "alt-svc": "h3=\":443\"; ma=93600",
    "x-message": "IP restriction applied, location not allowed.",
    "x-merchantid": "hyperswitch",
    "content-type": "application/json; charset=utf-8",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "cache-control": "max-age=0",
    "server-timing": "ak_p; desc=\"1774268281662_1752377261_278613465_28993_6210_8_20_-\";dur=1",
    "x-resultcode": "1578",
    "date": "Mon, 23 Mar 2026 12:18:01 GMT",
    "content-length": "230",
    "x-content-type-options": "nosniff",
    "x-apioperation": "INITIATE"
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
    "minor_amount": 3000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_9e77235b92204e13bbb38a3b2b60bc98"
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
    "x-message": "IP restriction applied, location not allowed.",
    "permissions-policy": "accelerometer=(), ambient-light-sensor=(), autoplay=(), battery=(), camera=(), fullscreen=(self), geolocation=*, gyroscope=(), magnetometer=(), microphone=(), midi=(), navigation-override=(), payment=*, picture-in-picture=*, publickey-credentials-get=*, usb=()",
    "x-content-type-options": "nosniff",
    "server-timing": "ak_p; desc=\"1774268282015_1752377261_278613590_26452_6640_8_17_-\";dur=1",
    "content-type": "application/json; charset=utf-8",
    "content-length": "230",
    "x-merchantid": "hyperswitch",
    "x-resultcode": "1578",
    "x-apioperation": "CAPTURE",
    "akamai-cache-status": "Miss from child, Miss from parent",
    "alt-svc": "h3=\":443\"; ma=93600",
    "np-waf-trace-id": "0.ad277368.1774268282.109b4e56",
    "date": "Mon, 23 Mar 2026 12:18:02 GMT",
    "connection": "close",
    "strict-transport-security": "max-age=15768000 ; includeSubDomains ; preload",
    "referrer-policy": "no-referrer-when-downgrade",
    "x-classdescription": "Rejected AccessDenied"
  }
}
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
