# Connector `stripe` / Suite `create_sdk_session_token` / Scenario `create_sdk_session_apple_pay`

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
x-connector-request-reference-id: create_sdk_session_token_create_sdk_session_apple_pay_ref
x-merchant-id: test_merchant
x-request-id: create_sdk_session_token_create_sdk_session_apple_pay_req
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
  -H "x-request-id: create_sdk_session_token_create_sdk_session_apple_pay_req" \
  -H "x-connector-request-reference-id: create_sdk_session_token_create_sdk_session_apple_pay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.MerchantAuthenticationService/CreateSdkSessionToken <<'JSON'
{
  "merchant_sdk_session_id": "gen_826141",
  "amount": {
    "minor_amount": 10000,
    "currency": "USD"
  },
  "payment_method_type": "APPLE_PAY",
  "country_alpha2_code": "US",
  "customer": {
    "name": "Mia Johnson",
    "email": {
      "value": "jordan.5038@sandbox.example.com"
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
x-connector-request-reference-id: create_sdk_session_token_create_sdk_session_apple_pay_ref
x-merchant-id: test_merchant
x-request-id: create_sdk_session_token_create_sdk_session_apple_pay_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateSdkSessionToken": ***MASKED***
```

</details>


[Back to Connector Suite](../create-sdk-session-token.md) | [Back to Overview](../../../test_overview.md)
