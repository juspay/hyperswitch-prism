# Connector `stripe` / Suite `create_sdk_session_token` / Scenario `create_sdk_session_with_taxes`

- Service: `Unknown`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Initialize wallet payment sessions for Apple Pay, Google Pay, etc.
// Sets up secure context for tokenized wallet payments with device verification.
rpc CreateSdkSessionToken ( .types.MerchantAuthenticationServiceCreateSdkSessionTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateSdkSessionTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_sdk_session_token_create_sdk_session_with_taxes_ref
x-merchant-id: test_merchant
x-request-id: create_sdk_session_token_create_sdk_session_with_taxes_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateSdkSessionToken": ***MASKED***
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_sdk_session_token_create_sdk_session_with_taxes_req" \
  -H "x-connector-request-reference-id: create_sdk_session_token_create_sdk_session_with_taxes_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.MerchantAuthenticationService/CreateSdkSessionToken <<'JSON'
{
  "merchant_sdk_session_id": "gen_821079",
  "amount": {
    "minor_amount": 15000,
    "currency": "USD"
  },
  "order_tax_amount": 1200,
  "shipping_cost": 500,
  "payment_method_type": "APPLE_PAY",
  "country_alpha2_code": "US",
  "customer": {
    "name": "Liam Smith",
    "email": {
      "value": "alex.4733@testmail.io"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initialize wallet payment sessions for Apple Pay, Google Pay, etc.
// Sets up secure context for tokenized wallet payments with device verification.
rpc CreateSdkSessionToken ( .types.MerchantAuthenticationServiceCreateSdkSessionTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateSdkSessionTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_sdk_session_token_create_sdk_session_with_taxes_ref
x-merchant-id: test_merchant
x-request-id: create_sdk_session_token_create_sdk_session_with_taxes_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateSdkSessionToken": ***MASKED***
```

</details>


[Back to Connector Suite](../create-sdk-session-token.md) | [Back to Overview](../../../test_overview.md)
