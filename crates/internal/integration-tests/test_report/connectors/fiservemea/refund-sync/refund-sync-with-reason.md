# Connector `fiservemea` / Suite `refund_sync` / Scenario `refund_sync_with_reason`

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
  "merchant_transaction_id": "mti_519b7283ab554259a584a9e2",
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
        "value": "Mia Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Wilson",
    "email": {
      "value": "casey.6046@testmail.io"
    },
    "id": "cust_d99d2316ea4e44d19469150a",
    "phone_number": "+914645390767"
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
        "value": "Brown"
      },
      "line1": {
        "value": "2084 Main Ln"
      },
      "line2": {
        "value": "420 Main Rd"
      },
      "line3": {
        "value": "5296 Market St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "64546"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.7868@testmail.io"
      },
      "phone_number": {
        "value": "6716240193"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "6724 Oak Rd"
      },
      "line2": {
        "value": "2756 Pine Ln"
      },
      "line3": {
        "value": "9682 Market Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "10467"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9815@example.com"
      },
      "phone_number": {
        "value": "6520348040"
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
date: Tue, 24 Mar 2026 07:01:12 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "af271edc-2ef7-4ad1-958d-e133c46983b0",
  "connectorTransactionId": "84649326315",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "af271edc-2ef7-4ad1-958d-e133c46983b0",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:01:12 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335672; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:01:12 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "279",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "491dc2a2-4497-49a7-ae77-f1b84fd6864a",
    "x-xss-protection": "1; mode=block"
  },
  "networkTransactionId": "acI2uGpn0g2Q4fANn_SLTQAAA60",
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
  "merchant_refund_id": "mri_b5d0e1ef539a413391bdc2f5",
  "connector_transaction_id": "84649326315",
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
date: Tue, 24 Mar 2026 07:01:13 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "84649326316",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "bb8363a8-2e5e-491a-aacc-74f6ea99bb71",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:01:13 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335673; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:01:13 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "264",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "094c1212-da56-4a02-a8df-91a9445d9be9",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "84649326315",
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
  -H "x-request-id: refund_sync_refund_sync_with_reason_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "84649326315",
  "refund_id": "84649326316",
  "connector_feature_data": {
    "value": "{\"token_reusable\":\"true\"}"
  },
  "refund_reason": "customer_requested"
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
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:01:14 GMT
x-request-id: refund_sync_refund_sync_with_reason_req

Response contents:
{
  "merchantRefundId": "84649326316",
  "connectorRefundId": "84649326316",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-headers": "Authorization, origin, x-requested-with, accept, content-type, Client-Request-Id, Api-Key, Timestamp, Message-Signature",
    "access-control-allow-methods": "GET, PUT, POST, DELETE, PATCH",
    "access-control-allow-origin": "https://prod.emea.api.fiservapps.com",
    "access-control-max-age": "3628800",
    "cache-control": "no-cache, no-store, must-revalidate",
    "client-request-id": "6cc9e2ac-7179-4df4-a0a5-b759ea98bb76",
    "connection": "keep-alive",
    "content-security-policy": "default-src 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.npci.org.in *.aws.fisv.cloud 'unsafe-eval' 'unsafe-inline'; frame-ancestors 'self'; connect-src 'self' *.aws.fisv.cloud",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:01:14 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "set-cookie": "__uzmd=1774335673; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:01:13 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=63072000; includeSubdomains",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-envoy-upstream-service-time": "117",
    "x-frame-options": "SAMEORIGIN",
    "x-request-id": "282405ea-3e76-441a-8ce2-75b9367ee73e",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "84649326315",
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
