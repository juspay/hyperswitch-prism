# Connector `paypal` / Suite `authorize` / Scenario `no3ds_auto_capture_bacs_bank_transfer`

- Service: `PaymentService/Authorize`
- PM / PMT: `bacs_bank_transfer` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. create_access_token(create_access_token) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_access_token_create_access_token_req" \
  -H "x-connector-request-reference-id: create_access_token_create_access_token_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.MerchantAuthenticationService/CreateAccessToken <<'JSON'
{
  "merchant_access_token_id": ***MASKED***"
  "connector": "STRIPE",
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Generate short-lived connector authentication token. Provides secure
// credentials for connector API access without storing secrets client-side.
rpc CreateAccessToken ( .types.MerchantAuthenticationServiceCreateAccessTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateAccessTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_access_token_create_access_token_ref
x-merchant-id: test_merchant
x-request-id: create_access_token_create_access_token_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:25:06 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
  },
  "expiresInSeconds": "32399",
  "status": "OPERATION_STATUS_SUCCESS",
  "statusCode": 200
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
  -H "x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bacs_bank_transfer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_d6e0233364bc4c9cb1eaa12d72a04e4f",
  "amount": {
    "minor_amount": 6000,
    "currency": "GBP"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "bacs_bank_transfer": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "riley.1033@testmail.io"
    },
    "id": "cust_fdad496ab603452dad97e7e89a853675",
    "phone_number": "+14759225676"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32399"
    }
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
        "value": "3434 Sunset Ln"
      },
      "line2": {
        "value": "2171 Sunset Ln"
      },
      "line3": {
        "value": "8650 Market St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "15762"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "morgan.9907@sandbox.example.com"
      },
      "phone_number": {
        "value": "8664900756"
      },
      "phone_country_code": "+44"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "3245 Lake Blvd"
      },
      "line2": {
        "value": "5314 Pine Blvd"
      },
      "line3": {
        "value": "8469 Main Blvd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "ENG"
      },
      "zip_code": {
        "value": "48321"
      },
      "country_alpha2_code": "GB",
      "email": {
        "value": "morgan.4376@sandbox.example.com"
      },
      "phone_number": {
        "value": "5883304500"
      },
      "phone_country_code": "+44"
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
  "description": "No3DS BACS bank transfer payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bacs_bank_transfer_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:25:08 GMT
x-request-id: authorize_no3ds_auto_capture_bacs_bank_transfer_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_IMPLEMENTED",
      "message": "This step has not been implemented for: Selected payment method through Paypal"
    }
  },
  "statusCode": 501,
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expiresInSeconds": "32399"
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
