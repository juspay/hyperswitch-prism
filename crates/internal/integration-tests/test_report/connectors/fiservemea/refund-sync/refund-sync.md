# Connector `fiservemea` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
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
  "merchant_transaction_id": "mti_d0a1009bdca64fcab98bf77f",
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
        "value": "Emma Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Brown",
    "email": {
      "value": "alex.1305@sandbox.example.com"
    },
    "id": "cust_4e71e530589047b1b690b7cf",
    "phone_number": "+444549873135"
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
        "value": "Smith"
      },
      "line1": {
        "value": "860 Sunset Rd"
      },
      "line2": {
        "value": "1484 Pine Ln"
      },
      "line3": {
        "value": "9907 Oak St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "87940"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2413@example.com"
      },
      "phone_number": {
        "value": "5126315008"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "8552 Lake Ave"
      },
      "line2": {
        "value": "6480 Sunset Dr"
      },
      "line3": {
        "value": "948 Pine St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "37149"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9825@example.com"
      },
      "phone_number": {
        "value": "6447738674"
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
date: Tue, 24 Mar 2026 07:01:09 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "b278bd31-1803-45a1-b8c4-812b5983835b",
  "connectorTransactionId": "84649326313",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "b278bd31-1803-45a1-b8c4-812b5983835b",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:01:09 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335669; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:01:09 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "270",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "40c12dbd-edd0-402f-9068-87fa187b62d1",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "acI2tWYIDXYTnyNnphjNsgAAAYY",
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
<summary>2. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_1e198595ae78455184c98faf",
  "connector_transaction_id": "84649326313",
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
<summary>Show Dependency Response (masked)</summary>

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
date: Tue, 24 Mar 2026 07:01:10 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "84649326314",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "35e1a8ad-286e-487b-867b-8fc76f36d11e",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:01:10 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335670; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:01:10 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "251",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "a7761f1b-85fa-45c3-b2df-fffc2e1a6810",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "84649326313",
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "84649326313",
  "refund_id": "84649326314",
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
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:01:11 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "merchantRefundId": "84649326314",
  "connectorRefundId": "84649326314",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "2059c6a0-3334-4268-b38f-76cdffc1c5d0",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:01:11 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335671; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:01:11 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "158",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "732d2b17-5221-4700-89cd-ce04cecd72fc",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "84649326313",
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
