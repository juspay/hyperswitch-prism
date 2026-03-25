# Connector `stax` / Suite `authorize` / Scenario `no3ds_manual_capture_debit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `debit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — PASS</summary>

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
  "merchant_customer_id": "mcui_6e4a231eff224aaea5ee05ca",
  "customer_name": "Emma Wilson",
  "email": {
    "value": "sam.6640@example.com"
  },
  "phone_number": "+918636591810",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "988 Sunset Blvd"
      },
      "line2": {
        "value": "7465 Market Ln"
      },
      "line3": {
        "value": "894 Market Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33355"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5007@example.com"
      },
      "phone_number": {
        "value": "5688434175"
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
        "value": "9721 Market Ave"
      },
      "line2": {
        "value": "8710 Sunset Rd"
      },
      "line3": {
        "value": "9881 Oak Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45671"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5804@example.com"
      },
      "phone_number": {
        "value": "3567416002"
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
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:40 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "585ea3f8-f739-40a0-b3bc-823ab3ce41c5",
  "connectorCustomerId": "585ea3f8-f739-40a0-b3bc-823ab3ce41c5",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fb9c9a6a4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:36 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=D0gihUY.KBR8Yggn1xGdbrQHLdnrLZ06UotHOh00QZg-1774337555.934949-1.0.1.1-uhmPZlAI6bMQFmej9gNpQRKV5GO4Rgb59FKzXu53n27NQIOYNe8ZtvJcgZWQk.e6UDf4p.cdsoZ6BrLKEdtpxnKTqM8svwLTFrDfBPN94IFpDamHB7oBDJGXCixUhKv3; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:36 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. tokenize_payment_method(tokenize_credit_card) — FAIL</summary>

**Dependency Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"422","message":"The selected customer id is invalid.","reason":"{\"customer_id\":[\"The selected customer id is invalid.\"]}"}}
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_credit_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_675675",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "John Doe"
      }
    }
  },
  "customer": {
    "id": "cust_8903bf6f9076430fa4f1f492",
    "name": "Mia Wilson",
    "email": {
      "value": "casey.9648@example.com"
    },
    "connector_customer_id": "585ea3f8-f739-40a0-b3bc-823ab3ce41c5"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "9721 Market Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45671"
      },
      "country_alpha2_code": "US"
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
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_credit_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:41 GMT
x-request-id: tokenize_payment_method_tokenize_credit_card_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "422",
      "message": "The selected customer id is invalid.",
      "reason": "{\"customer_id\":[\"The selected customer id is invalid.\"]}"
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fbba1b524734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:41 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=As86M_aBivNcEHlXkZ9uxIQhPTIQhj58cSPQ8fThWlo-1774337560.6588912-1.0.1.1-OX.DsKMb08rAdyiQnX0TGeNnJ4UrM7rOPgks7tcAuTnPWPbZ6cw61z6dxhraY6WP.UeMLPse16g2ZxZYmYHTXG.ibEnWupqdhUoQwKt6CoGLt.x4IsN3CwQ13v96Mm0X; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:41 GMT",
    "transfer-encoding": "chunked",
    "vary": "accept-encoding",
    "x-powered-by": "PHP/8.3.11"
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
  -H "x-request-id: authorize_no3ds_manual_capture_debit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_debit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_62b56acbac244f299a962be5",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "John Doe"
      },
      "card_type": "debit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Mia Wilson",
    "email": {
      "value": "casey.9648@example.com"
    },
    "id": "cust_8903bf6f9076430fa4f1f492",
    "phone_number": "+17866480729",
    "connector_customer_id": "585ea3f8-f739-40a0-b3bc-823ab3ce41c5"
  },
  "payment_method_token": ***MASKED***
    "value": ""
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "988 Sunset Blvd"
      },
      "line2": {
        "value": "7465 Market Ln"
      },
      "line3": {
        "value": "894 Market Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "33355"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.5007@example.com"
      },
      "phone_number": {
        "value": "5688434175"
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
        "value": "9721 Market Ave"
      },
      "line2": {
        "value": "8710 Sunset Rd"
      },
      "line3": {
        "value": "9881 Oak Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45671"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.5804@example.com"
      },
      "phone_number": {
        "value": "3567416002"
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
  "description": "No3DS manual capture card payment (debit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US",
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
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_manual_capture_debit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_debit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:42 GMT
x-request-id: authorize_no3ds_manual_capture_debit_card_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "422",
      "message": "The payment method id field is required.",
      "reason": "{\"payment_method_id\":[\"The payment method id field is required.\"]}"
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fbc0f8444734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:42 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=6.lAItJkdS_x3gmPtZMLStMIDvupbND2K5GtYpbPUCs-1774337561.753991-1.0.1.1-HD2UA1JcT5MxSnqTGIXlGAfl4ih7nMLuxiIZQ75s2yKliRNe2eopQgPpshKYPeYdViN0kCUrkGAo4T43ZmDalnbAJOuEa7h4EaZHN6omowW6YR1XnXtRB6uVv2fdPCrq; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:42 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
  },
  "state": {
    "connectorCustomerId": "585ea3f8-f739-40a0-b3bc-823ab3ce41c5"
  },
  "rawConnectorResponse": "***MASKED***"
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
