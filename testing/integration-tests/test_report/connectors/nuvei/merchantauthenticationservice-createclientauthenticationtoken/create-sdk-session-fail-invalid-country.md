# Connector `nuvei` / Suite `MerchantAuthenticationService/CreateClientAuthenticationToken` / Scenario `Create Client Authentication Token | Create SDK Session Fail Invalid Country`

- Service: `Unknown`
- Scenario Key: `create_sdk_session_fail_invalid_country`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Initialize client-facing SDK sessions for wallets, device fingerprinting,
// etc. Returns structured data the client SDK needs to render
// payment/verification UI.
rpc CreateClientAuthenticationToken ( .types.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_fail_invalid_country_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_fail_invalid_country_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateClientAuthenticationToken": ***MASKED***"
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_fail_invalid_country_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_fail_invalid_country_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.MerchantAuthenticationService/CreateClientAuthenticationToken <<'JSON'
{
  "merchant_client_session_id": "gen_334630",
  "payment": {
    "amount": {
      "minor_amount": 10000,
      "currency": "USD"
    },
    "payment_method_type": "APPLE_PAY",
    "country_alpha2_code": "XX"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initialize client-facing SDK sessions for wallets, device fingerprinting,
// etc. Returns structured data the client SDK needs to render
// payment/verification UI.
rpc CreateClientAuthenticationToken ( .types.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_fail_invalid_country_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_fail_invalid_country_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateClientAuthenticationToken": ***MASKED***"
```

</details>


[Back to Connector Suite](../merchantauthenticationservice-createclientauthenticationtoken.md) | [Back to Overview](../../../test_overview.md)
