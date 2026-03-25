# Connector `stax` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
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
  "merchant_customer_id": "mcui_08d392ec65464a4d9b448083",
  "customer_name": "Ava Johnson",
  "email": {
    "value": "alex.1285@example.com"
  },
  "phone_number": "+16307353645",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "225 Oak Rd"
      },
      "line2": {
        "value": "4451 Market Ln"
      },
      "line3": {
        "value": "6840 Lake St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "61720"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2563@testmail.io"
      },
      "phone_number": {
        "value": "8102459129"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3004 Oak St"
      },
      "line2": {
        "value": "4992 Main Ln"
      },
      "line3": {
        "value": "5644 Pine St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "10066"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6452@testmail.io"
      },
      "phone_number": {
        "value": "7345475354"
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
date: Tue, 24 Mar 2026 07:33:04 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "472ca725-d3aa-4634-8ab1-92e1e9460274",
  "connectorCustomerId": "472ca725-d3aa-4634-8ab1-92e1e9460274",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fc4a8d5f4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:33:04 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=TFiHZ11b27_CbMRaQNelBkUOLyETVlawqwbpbgDxRi8-1774337583.7693305-1.0.1.1-ofJosaL6c6.k5dNk4jvOCR9vDAoKXqItUM23b35.XSLevyJWTgdvJIUyjqr1yyc6dx1mj0Poq09LVroByvtkuXXWuBHkVniy40Q0JAB_r_46ae4YXXlQmuyTinQJqcZK; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:03:04 GMT",
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
  "merchant_payment_method_id": "gen_572764",
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
    "id": "cust_567024c88931411da227f3c2",
    "name": "Ethan Miller",
    "email": {
      "value": "jordan.7229@example.com"
    },
    "connector_customer_id": "472ca725-d3aa-4634-8ab1-92e1e9460274"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3004 Oak St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "10066"
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
date: Tue, 24 Mar 2026 07:33:06 GMT
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
    "cf-ray": "9e13fc4fd8ee4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:33:05 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=C2eOwin2A42VyRBeEIf4Z5Q9NLooAIvM1qwiQa48UeU-1774337584.6202302-1.0.1.1-.zWaWptpHPFVakcHDiCCWf.9okmRWvNLyWMnK.TYWswNbvCmuqcVC.f0TQvMyUGE5bSs9ry7_wEXj8GJaQilm3XWgLgfBWt8KUR5Yk.JJgigVRVVjVD1PUkteY1Vqbxq; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:03:05 GMT",
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
<summary>3. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

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
  "merchant_transaction_id": "mti_5e2d7ce27d684fb5a3d8244b",
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
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "jordan.7229@example.com"
    },
    "id": "cust_567024c88931411da227f3c2",
    "phone_number": "+15821478050",
    "connector_customer_id": "472ca725-d3aa-4634-8ab1-92e1e9460274"
  },
  "payment_method_token": ***MASKED***
    "value": ""
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "225 Oak Rd"
      },
      "line2": {
        "value": "4451 Market Ln"
      },
      "line3": {
        "value": "6840 Lake St"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "61720"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.2563@testmail.io"
      },
      "phone_number": {
        "value": "8102459129"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3004 Oak St"
      },
      "line2": {
        "value": "4992 Main Ln"
      },
      "line3": {
        "value": "5644 Pine St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "10066"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.6452@testmail.io"
      },
      "phone_number": {
        "value": "7345475354"
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
date: Tue, 24 Mar 2026 07:33:07 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

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
    "cf-ray": "9e13fc5b89314734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:33:06 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=tilHPTJ7i4J1beZCTL6e4S_tD4DdiAH7kO2Q2JE4b_A-1774337586.4863992-1.0.1.1-7J1cvOlTQWG8495B.K8RQMwX4C8a9VenCbnXDUAAj3tvpZItgbR.DKinwBG8AUqyD3HGKiAii367mrc1LAQ0lkH2.IyF90jvZEZlO8fAV0KG2gbd6uTzLcsKOca5Y2Vn; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:03:06 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
  },
  "state": {
    "connectorCustomerId": "472ca725-d3aa-4634-8ab1-92e1e9460274"
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

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "472ca725-d3aa-4634-8ab1-92e1e9460274"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:33:08 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "422",
      "message": "The selected id is invalid.",
      "reason": "{\"id\":[\"The selected id is invalid.\"]}"
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fc645f7e4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:33:08 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=5OG86iwVcE5Hvd3DESz5P5p_HbdxukTV8oxQ35oefvw-1774337587.894711-1.0.1.1-7rw9ZatwZvUR4HyJq4CHCosj8_dohCbmjij4uVBmXeZ0iRm6dqqlRAKtam8gI_MU340PdrfMAg.In61M93dNzHkNAEUJd3FzX.P45NFaYkz3A.S1dsIvkdunpkj2dqR.; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:03:08 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
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


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
