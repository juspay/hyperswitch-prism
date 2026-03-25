# Connector `stax` / Suite `capture` / Scenario `capture_with_merchant_order_id`

- Service: `PaymentService/Capture`
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
  "merchant_customer_id": "mcui_ad5e8998da8d49d2b544ff1a",
  "customer_name": "Mia Miller",
  "email": {
    "value": "casey.5858@example.com"
  },
  "phone_number": "+443998463957",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "6287 Main Blvd"
      },
      "line2": {
        "value": "1231 Oak Dr"
      },
      "line3": {
        "value": "8772 Market Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83003"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4024@testmail.io"
      },
      "phone_number": {
        "value": "4614032660"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1163 Lake Ave"
      },
      "line2": {
        "value": "8453 Main Ave"
      },
      "line3": {
        "value": "5849 Lake Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99588"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1302@example.com"
      },
      "phone_number": {
        "value": "6909821831"
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
date: Tue, 24 Mar 2026 07:32:55 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "c8ec521d-dab5-4989-982c-a614a73f76a7",
  "connectorCustomerId": "c8ec521d-dab5-4989-982c-a614a73f76a7",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fc16380c4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:55 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=p_MSy_onJkwg.tOZrKlkys10q2RaZc1TRknrABG834I-1774337575.395984-1.0.1.1-nk3P6FmICa7DYshKwpij111F7O3Oi2JRg_RhG_VXMMPHn8Z_zXnGYZ_ThZeNt4ANAVVUniJB2Ohdc3sTEckJBJtDqwyECdKVK4XMIGcjnKYquyukXkvKPt_jYth9fjZe; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:55 GMT",
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
  "merchant_payment_method_id": "gen_940340",
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
    "id": "cust_96d6aa3dcc6b44488e75cfb9",
    "name": "Noah Miller",
    "email": {
      "value": "casey.2548@example.com"
    },
    "connector_customer_id": "c8ec521d-dab5-4989-982c-a614a73f76a7"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1163 Lake Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99588"
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
date: Tue, 24 Mar 2026 07:32:57 GMT
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
    "cf-ray": "9e13fc1b9bd84734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:57 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=8h.MfsrbBp7xALuNzEsUMpSCIyOvNi5m14MJPVLRphQ-1774337576.2515295-1.0.1.1-E5eoXwjSJl4raukQHa0AAmxsVV8vmKXf9d1ZPprQv9_cudEI13uYTIS6Os2cDvjoFcC9HXbqv6RC.sgRAw7rP2e.fXVuh7hGJTtkEK9stVkKMBJ1dG28JZBuyopugGIn; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:57 GMT",
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
<summary>3. authorize(no3ds_manual_capture_credit_card) — FAIL</summary>

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
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_e50a33ecc86c43e49f633783",
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
  "capture_method": "MANUAL",
  "customer": {
    "name": "Noah Miller",
    "email": {
      "value": "casey.2548@example.com"
    },
    "id": "cust_96d6aa3dcc6b44488e75cfb9",
    "phone_number": "+18071584088",
    "connector_customer_id": "c8ec521d-dab5-4989-982c-a614a73f76a7"
  },
  "payment_method_token": ***MASKED***
    "value": ""
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
        "value": "6287 Main Blvd"
      },
      "line2": {
        "value": "1231 Oak Dr"
      },
      "line3": {
        "value": "8772 Market Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83003"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4024@testmail.io"
      },
      "phone_number": {
        "value": "4614032660"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "1163 Lake Ave"
      },
      "line2": {
        "value": "8453 Main Ave"
      },
      "line3": {
        "value": "5849 Lake Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "99588"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1302@example.com"
      },
      "phone_number": {
        "value": "6909821831"
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:58 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

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
    "cf-ray": "9e13fc246a8a4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:58 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=VMplD21XyTLBlK_XQguRImhFT.HpVz1rRtKsheKeub0-1774337577.6684463-1.0.1.1-nVbv0s_KUdi2wXNc_W_rVxMt9MVTOKoYA5clEIx0_PKXBLnhsuzliKHaS0G_rhG4RgUWYEMFYPeO6CzVbec1g5L7TG_leUxIq7pxkdjAClB0Q0NDkJxrf_nwzsGCEJ9R; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:58 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
  },
  "state": {
    "connectorCustomerId": "c8ec521d-dab5-4989-982c-a614a73f76a7"
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "auto_generate",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_86959ef970d84a8685ba3fb8",
  "merchant_order_id": "gen_659818",
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
  "state": {
    "connector_customer_id": "c8ec521d-dab5-4989-982c-a614a73f76a7"
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
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:59 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "error": {
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
    "cf-ray": "9e13fc2abf8f4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:59 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=Io4jiqp_Wp65iZrI_fcWECZzHWRndRNK.yVvud3N4NI-1774337578.672793-1.0.1.1-hwkKQBexikvhBE6a44VddRhOMAbaQIJzS.A3ar_fAwu23QGJo2DMSmNdR0nwVGH41M2M7zWJp4aN9CiWH4BW3o.dJQEVP81G6d13eOHZ4Wr9QWEPbOOLZO0Q5s9ozO4o; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:59 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
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
