# Connector `xendit` / Suite `void` / Scenario `void_with_amount`

- Service: `PaymentService/Void`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk HTTP request failed for 'void'/'void_with_amount': builder error
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
  "merchant_transaction_id": "mti_e5d54d5bc71d411c9ca01dfb2b7d4fec",
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
        "value": "Mia Johnson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Brown",
    "email": {
      "value": "jordan.6624@sandbox.example.com"
    },
    "id": "cust_0e45ce85d84f464fbc6bd8620e28284d",
    "phone_number": "+916104404743"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "362 Market Ln"
      },
      "line2": {
        "value": "1306 Oak St"
      },
      "line3": {
        "value": "2457 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67633"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.2205@testmail.io"
      },
      "phone_number": {
        "value": "9976267574"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "1594 Lake Blvd"
      },
      "line2": {
        "value": "1997 Market Rd"
      },
      "line3": {
        "value": "4170 Pine Ln"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "16003"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1218@example.com"
      },
      "phone_number": {
        "value": "1255385524"
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
    "rate-limit-reset": "49.966",
    "server": "cloudflare",
    "content-length": "90",
    "request-id": "69c13324000000006bf154c8b83c2d4e",
    "set-cookie": "__cf_bm=CW7UOdpUjcLkmNbDuGLBhfFoAUdz0_eV80AvmksdOhI-1774269220.8755195-1.0.1.1-_Kna6SEl3GO2OCM1IVt.5NIPD6Yxy4Izy4HXltfn.fqXzcFupDyGUolZp90dw5ngvSLPobkFxvHyyIgxy7zYsjdLOmvMo7C3trcCVPeQAyfITKcd4_wHrHqSXJw3jj9q; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Mon, 23 Mar 2026 13:03:41 GMT",
    "cf-cache-status": "DYNAMIC",
    "rate-limit-remaining": "45",
    "content-type": "application/json",
    "x-envoy-upstream-service-time": "251",
    "rate-limit-limit": "60",
    "access-control-allow-origin": "*",
    "vary": "Origin",
    "cf-ray": "9e0d77467a7a8602-BOM",
    "date": "Mon, 23 Mar 2026 12:33:41 GMT",
    "connection": "keep-alive"
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
