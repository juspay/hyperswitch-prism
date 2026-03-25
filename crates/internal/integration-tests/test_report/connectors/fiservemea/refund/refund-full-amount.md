# Connector `fiservemea` / Suite `refund` / Scenario `refund_full_amount`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_5d82638e6dc04a0bae8826ac",
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
        "value": "Emma Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Johnson",
    "email": {
      "value": "morgan.6175@testmail.io"
    },
    "id": "cust_54cd023a5edc474d84c7e257",
    "phone_number": "+13117048032"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1854 Market Dr"
      },
      "line2": {
        "value": "3575 Main Rd"
      },
      "line3": {
        "value": "4751 Oak Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "73129"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5449@testmail.io"
      },
      "phone_number": {
        "value": "6316511014"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4356 Oak Ln"
      },
      "line2": {
        "value": "8182 Sunset St"
      },
      "line3": {
        "value": "2192 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "50438"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5393@example.com"
      },
      "phone_number": {
        "value": "2764463971"
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
  "description": "No3DS auto capture card payment (credit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:00:54 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "d470ce43-12d0-4570-a5e5-abd815ecd93c",
  "connectorTransactionId": "84649326295",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "d470ce43-12d0-4570-a5e5-abd815ecd93c",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:00:54 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335654; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:00:54 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "251",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "5baa1c93-9f2b-4839-ae4e-2910f9d86ad9",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "acI2pmYIDXYTnyNnphjNfQAAAY4",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorFeatureData": {
    "value": "{\"token_reusable\":\"true\"}"
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
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_b2b317e6a2cc46488cb1f534",
  "connector_transaction_id": "84649326295",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_feature_data": {
    "value": "{\"token_reusable\":\"true\"}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:00:55 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "84649326296",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "9d275e2d-b8b1-4357-9e80-31a5f7fc17b4",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:00:55 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335655; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:00:55 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "302",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "9809a420-e265-41b1-9b38-5e24e5656658",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "84649326295",
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


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
