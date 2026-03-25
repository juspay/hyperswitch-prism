# Connector `powertranz` / Suite `capture` / Scenario `capture_partial_amount`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_89cbdcaf01ed4deba850c978",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "999"
      },
      "card_holder_name": {
        "value": "Mia Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Smith",
    "email": {
      "value": "jordan.1711@testmail.io"
    },
    "id": "cust_40f1b4e7bf624f45983e5ac9",
    "phone_number": "+442959672989"
  },
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1541 Main Rd"
      },
      "line2": {
        "value": "8341 Market Ln"
      },
      "line3": {
        "value": "8992 Oak St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18196"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.6991@testmail.io"
      },
      "phone_number": {
        "value": "8826072005"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4183 Oak Rd"
      },
      "line2": {
        "value": "2944 Oak St"
      },
      "line3": {
        "value": "8814 Main Ave"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "65823"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.2526@example.com"
      },
      "phone_number": {
        "value": "4191461445"
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
  "test_mode": true,
  "locale": "en-US"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:06:30 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "connectorTransactionId": "41ea9eb3-96fc-4544-8908-a923712b4cfc",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d55a3c3447e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:30 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=xGPpXhPBZlH9ENH1d1mPwrGKuJ7ywCbVtp_1mzpC8fc-1774335988.837828-1.0.1.1-r5nZ03VBd7.enrpwYDM9mjfHD2NErKCJ84fUlWTLSMTJ6u4mk34q1c4NMLsDpsQabMEMiw.kQyZZNEAvuCcZW2vegdJMF40bgNLLmvKfr7RnwefqDmB891v2XK.PxIMH; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:30 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: capture_capture_partial_amount_req" \
  -H "x-connector-request-reference-id: capture_capture_partial_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "41ea9eb3-96fc-4544-8908-a923712b4cfc",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_28d6774d9dfc49f59cd0fdd7",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_partial_amount_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_partial_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:06:31 GMT
x-request-id: capture_capture_partial_amount_req

Response contents:
{
  "connectorTransactionId": "312e414b-37d9-4678-b3e7-d0ef12fb5a4b",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13d562cc0447e6-BOM",
    "connection": "keep-alive",
    "content-security-policy": "default-src https: 'unsafe-eval' 'unsafe-inline'",
    "content-type": "application/json; charset=utf-8",
    "date": "Tue, 24 Mar 2026 07:06:31 GMT",
    "referrer-policy": "no-referrer-when-downgrade",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=LPR_SO4upMR2rvvs8SVd2aayTr8IL7I2XXovUsX2dRA-1774335990.202261-1.0.1.1-0.WMW0QGllyBy4dTSgRUeKwaSMrDhZUiYkveNOAA8IpU.8NiAdL2Mvno1yuH6joOOwAzmtq.KOkaHARkVkXqkNiMjdVY8GzgI74BoFw3yP18JV1bPPtbJVgNP0oFCrLd; HttpOnly; Secure; Path=/; Domain=ptranz.com; Expires=Tue, 24 Mar 2026 07:36:31 GMT",
    "strict-transport-security": "max-age=31536000; includeSubdomains=true",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-xss-protection": "1; mode=block"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
