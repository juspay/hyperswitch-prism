# Connector `worldpay` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"endpointNotFound","message":"Requested endpoint was not found."}}
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — FAIL</summary>

**Dependency Error**

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:12:38 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_4551c27d858e41b0a8fa2af7",
  "customer_name": "Liam Brown",
  "email": {
    "value": "sam.2238@example.com"
  },
  "phone_number": "+447889352787",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9360 Main Ln"
      },
      "line2": {
        "value": "8190 Sunset Ln"
      },
      "line3": {
        "value": "6289 Pine Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "49046"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3926@example.com"
      },
      "phone_number": {
        "value": "7460982506"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6879 Sunset Rd"
      },
      "line2": {
        "value": "6415 Market Ln"
      },
      "line3": {
        "value": "9753 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "57989"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9425@sandbox.example.com"
      },
      "phone_number": {
        "value": "1757665459"
      },
      "phone_country_code": "+91"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:12:38 GMT
x-request-id: create_customer_create_customer_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Connector customer creation failed: InternalServerError
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

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
  "merchant_transaction_id": "mti_521ed05201394baaa6980f44",
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
    "name": "Emma Smith",
    "email": {
      "value": "alex.4604@sandbox.example.com"
    },
    "id": "cust_75aa68fd24da44ec8ba1e832",
    "phone_number": "+915726231377"
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
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9360 Main Ln"
      },
      "line2": {
        "value": "8190 Sunset Ln"
      },
      "line3": {
        "value": "6289 Pine Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "49046"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3926@example.com"
      },
      "phone_number": {
        "value": "7460982506"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6879 Sunset Rd"
      },
      "line2": {
        "value": "6415 Market Ln"
      },
      "line3": {
        "value": "9753 Sunset Rd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "57989"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9425@sandbox.example.com"
      },
      "phone_number": {
        "value": "1757665459"
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
date: Tue, 24 Mar 2026 07:12:38 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "BAD_REQUEST",
      "message": "Invalid Configuration: connector_config.merchant_name"
    }
  },
  "statusCode": 400
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>3. refund(refund_full_amount) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'connector_refund_id': expected field to exist
```

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
  "merchant_refund_id": "mri_9317aa4014c4495e9af8c4d2",
  "connector_transaction_id": "auto_generate",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
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
date: Tue, 24 Mar 2026 07:12:39 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "status": 21,
  "error": {
    "connectorDetails": {
      "code": "urlContainsInvalidValue",
      "message": "Please provide a valid url value or values."
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "cache-control": "no-cache, no-store, max-age=0, must-revalidate",
    "connection": "close",
    "content-length": "95",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:12:39 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "referrer-policy": "no-referrer",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "wp-correlationid": "70dd795a-8ee3-4b51-8886-4eed7d074316",
    "x-cache-hits": "0, 0",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-served-by": "cache-lcy-egml8630029-LCY, cache-fra-etou8220130-FRA",
    "x-timer": "S1774336359.975434,VS0,VE88",
    "x-xss-protection": "0"
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "auto_generate"
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
date: Tue, 24 Mar 2026 07:12:39 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "status": 21,
  "error": {
    "connectorDetails": {
      "code": "endpointNotFound",
      "message": "Requested endpoint was not found."
    }
  },
  "statusCode": 404,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "cache-control": "no-cache, no-store, max-age=0, must-revalidate",
    "connection": "keep-alive",
    "content-length": "78",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:12:39 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "referrer-policy": "no-referrer",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "wp-correlationid": "2951f0af-0573-48e8-b1f5-fb01ebc6c9c0",
    "x-cache-hits": "0, 0",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-served-by": "cache-lcy-egml8630083-LCY, cache-fra-etou8220124-FRA",
    "x-timer": "S1774336360.755073,VS0,VE89",
    "x-xss-protection": "0"
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


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
