# Connector `stax` / Suite `authorize` / Scenario `no3ds_fail_payment`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error.connector_details.message': expected 'The payment method id field is required.' to contain 'decline'
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
  "merchant_customer_id": "mcui_678c4ac6c18c46f7afd6e44d",
  "customer_name": "Ava Smith",
  "email": {
    "value": "morgan.8106@example.com"
  },
  "phone_number": "+441303273911",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4143 Sunset St"
      },
      "line2": {
        "value": "5821 Oak St"
      },
      "line3": {
        "value": "2018 Market Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60325"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6458@testmail.io"
      },
      "phone_number": {
        "value": "4107423261"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4732 Pine Rd"
      },
      "line2": {
        "value": "3494 Pine St"
      },
      "line3": {
        "value": "3395 Oak St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53761"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3730@testmail.io"
      },
      "phone_number": {
        "value": "8636671630"
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
date: Tue, 24 Mar 2026 07:32:30 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "e5b2a3f8-ae80-4560-a3b6-90f9290ebc23",
  "connectorCustomerId": "e5b2a3f8-ae80-4560-a3b6-90f9290ebc23",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fb79583c4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:30 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=yJvQECjPmEMVWHuc4aPeAq77cs3TMoMgzbmZ7Cfxda8-1774337550.2930212-1.0.1.1-m_068yhx.wAvd34vyvw57wke4ijqpbaNfVJkOooNG7wratv.B1T6cIXtn9QjwC3JP0jYE0Kxw9qmYv19wJLInK143SZN1xisUaWB7H3Zu7o2oAW0k.NZg9RIuKDpDzj.; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:30 GMT",
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
  "merchant_payment_method_id": "gen_935942",
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
    "id": "cust_2905a21941dd4220901d589b",
    "name": "Noah Smith",
    "email": {
      "value": "casey.1188@example.com"
    },
    "connector_customer_id": "e5b2a3f8-ae80-4560-a3b6-90f9290ebc23"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4732 Pine Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53761"
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
date: Tue, 24 Mar 2026 07:32:32 GMT
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
    "cf-ray": "9e13fb7ebced4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:32 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=NolhJbfA.aaLuZYCYhk5PI8F_WwtN8PTGfuDSptdHIo-1774337551.1523879-1.0.1.1-AavV4lVkU2Vi4boNsnWQuKun3xYhjKsSbBMJYOZEVT_RqxjdRe5r4puMAY6mwcKhjNYKRbcdJObFJ_mJ1BWFnHYR5ZCUf9wdlurHHgP8LvwM69DMb9rCx6Lh1wgcsm25; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:32 GMT",
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
  -H "x-request-id: authorize_no3ds_fail_payment_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_fail_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_21a81fa984b0410baf1a238b",
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
    "name": "Noah Smith",
    "email": {
      "value": "casey.1188@example.com"
    },
    "id": "cust_2905a21941dd4220901d589b",
    "phone_number": "+911454148775",
    "connector_customer_id": "e5b2a3f8-ae80-4560-a3b6-90f9290ebc23"
  },
  "payment_method_token": ***MASKED***
    "value": ""
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
        "value": "4143 Sunset St"
      },
      "line2": {
        "value": "5821 Oak St"
      },
      "line3": {
        "value": "2018 Market Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60325"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.6458@testmail.io"
      },
      "phone_number": {
        "value": "4107423261"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "4732 Pine Rd"
      },
      "line2": {
        "value": "3494 Pine St"
      },
      "line3": {
        "value": "3395 Oak St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "53761"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.3730@testmail.io"
      },
      "phone_number": {
        "value": "8636671630"
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
  "description": "No3DS fail payment flow",
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
x-connector-request-reference-id: authorize_no3ds_fail_payment_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_fail_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:32 GMT
x-request-id: authorize_no3ds_fail_payment_req

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
    "cf-ray": "9e13fb868a824734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:32 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=qEgivHGyATLfK5jUT1UU9s_zMWmaiixc3YvitE.bWgk-1774337552.4027345-1.0.1.1-rB8JntSHgWSTdqcC6zSwGAvuDF4uRx9KYg90pCu4YVXwNJOC7kQcLzY1yt9KHFNiKdJr4jZLTTv6VXT7s7kVHPjuGFND.ParKPt0dyUUHX33af4oPheExs.ubWOMr7Pg; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:32 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
  },
  "state": {
    "connectorCustomerId": "e5b2a3f8-ae80-4560-a3b6-90f9290ebc23"
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
