# Connector `stax` / Suite `authorize` / Scenario `no3ds_auto_capture_sepa_bank_transfer`

- Service: `PaymentService/Authorize`
- PM / PMT: `sepa_bank_transfer` / `-`
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
  "merchant_customer_id": "mcui_c5d8b83f690e4b168ec614a6",
  "customer_name": "Noah Miller",
  "email": {
    "value": "morgan.7472@testmail.io"
  },
  "phone_number": "+14318529252",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "6881 Lake Ln"
      },
      "line2": {
        "value": "3133 Pine St"
      },
      "line3": {
        "value": "545 Lake Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35891"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2933@example.com"
      },
      "phone_number": {
        "value": "9661021438"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1584 Market St"
      },
      "line2": {
        "value": "6789 Pine St"
      },
      "line3": {
        "value": "5682 Market St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85433"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1513@example.com"
      },
      "phone_number": {
        "value": "8507530401"
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
date: Tue, 24 Mar 2026 07:32:28 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "c5a77713-bda5-431a-b70d-3007f238567d",
  "connectorCustomerId": "c5a77713-bda5-431a-b70d-3007f238567d",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fb60edd44734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:26 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=Vpkx5NHit49IOpq2Oxh2WutevCNCocDK0NR8OPUDtZE-1774337546.3818796-1.0.1.1-IWXRUn9awJVIT3ohkiWcYjFErnE.gUpgm3f3uTOs7uOGXKw0ia7sDEKZILaOVwDTuq4XM3DDrMbMoa.jDr_d.f1dzh5Ap06rBFFLdBwHuGmqtDvvKS.9bxVH5zpAUpdI; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:26 GMT",
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
  "merchant_payment_method_id": "gen_901662",
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
    "id": "cust_b060d00bce3745b18d991adc",
    "name": "Ethan Brown",
    "email": {
      "value": "alex.8129@testmail.io"
    },
    "connector_customer_id": "c5a77713-bda5-431a-b70d-3007f238567d"
  },
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1584 Market St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85433"
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
date: Tue, 24 Mar 2026 07:32:29 GMT
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
    "cf-ray": "9e13fb6f989d4734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:32:29 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=nbECaetENxW1QSFcKx2G0Sus9SCLVhJlyaoBAHDcCLk-1774337548.7387874-1.0.1.1-uSPmLYvm8wiuBEbVlMRjhnOsZaCg15d5EKtHC11Wu3ZAouY1sKyqj_pP3d01X4XAl9LIMZKBUqwkPXZaDwpJ50rZqG0cek7uyNIOScmwea0CLLgSGbgY.4qfqjfoCesG; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:02:29 GMT",
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
  -H "x-request-id: authorize_no3ds_auto_capture_sepa_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_sepa_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_55fe933ff28743debc4a39b2",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "sepa_bank_transfer": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Brown",
    "email": {
      "value": "alex.8129@testmail.io"
    },
    "id": "cust_b060d00bce3745b18d991adc",
    "phone_number": "+17529281820",
    "connector_customer_id": "c5a77713-bda5-431a-b70d-3007f238567d"
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
        "value": "Taylor"
      },
      "line1": {
        "value": "6881 Lake Ln"
      },
      "line2": {
        "value": "3133 Pine St"
      },
      "line3": {
        "value": "545 Lake Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35891"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.2933@example.com"
      },
      "phone_number": {
        "value": "9661021438"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "1584 Market St"
      },
      "line2": {
        "value": "6789 Pine St"
      },
      "line3": {
        "value": "5682 Market St"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85433"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1513@example.com"
      },
      "phone_number": {
        "value": "8507530401"
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
  "description": "No3DS SEPA bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_sepa_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_sepa_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:32:30 GMT
x-request-id: authorize_no3ds_auto_capture_sepa_bank_transfer_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_IMPLEMENTED",
      "message": "This step has not been implemented for: Only card and ACH bank debit payments are supported for Stax"
    }
  },
  "statusCode": 501,
  "state": {
    "connectorCustomerId": "c5a77713-bda5-431a-b70d-3007f238567d"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
