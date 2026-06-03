# Connector `nuvei` / Suite `MerchantAuthenticationService/CreateServerSessionAuthenticationToken` / Scenario `Create Session Token | Create Session Fail Invalid Currency`

- Service: `Unknown`
- Scenario Key: `create_session_fail_invalid_currency`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Create a server-side session with the connector. Establishes session state
// for multi-step operations like 3DS verification or wallet authorization.
rpc CreateServerSessionAuthenticationToken ( .types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_fail_invalid_currency_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_fail_invalid_currency_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken": ***MASKED***"
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_fail_invalid_currency_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_fail_invalid_currency_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken <<'JSON'
{
  "test_mode": true,
  "merchant_server_session_id": "gen_120408",
  "payment": {
    "amount": {
      "minor_amount": 10000,
      "currency": "XXX"
    }
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Create a server-side session with the connector. Establishes session state
// for multi-step operations like 3DS verification or wallet authorization.
rpc CreateServerSessionAuthenticationToken ( .types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateServerSessionAuthenticationTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_fail_invalid_currency_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_fail_invalid_currency_req
x-tenant-id: default

Error invoking method "types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken": ***MASKED***"
```

</details>


[Back to Connector Suite](../merchantauthenticationservice-createserversessionauthenticationtoken.md) | [Back to Overview](../../../test_overview.md)
