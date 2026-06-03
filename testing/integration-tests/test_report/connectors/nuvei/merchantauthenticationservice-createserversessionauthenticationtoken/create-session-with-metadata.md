# Connector `nuvei` / Suite `MerchantAuthenticationService/CreateServerSessionAuthenticationToken` / Scenario `Create Session Token | Create Session With Metadata`

- Service: `Unknown`
- Scenario Key: `create_session_with_metadata`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_with_metadata_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_with_metadata_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateServerSessionAuthenticationToken <<'JSON'
{
  "test_mode": true,
  "merchant_server_session_id": "gen_792326",
  "payment": {
    "amount": {
      "minor_amount": 15000,
      "currency": "USD"
    },
    "metadata": {
      "value": "{\"session_type\":\"checkout\",\"device\":\"mobile\"}"
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
x-connector-request-reference-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_with_metadata_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_with_metadata_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 10 Apr 2026 21:21:14 GMT
x-request-id: MerchantAuthenticationService/CreateServerSessionAuthenticationToken_create_session_with_metadata_req

Response contents:
{
  "statusCode": 200,
  "sessionToken": ***MASKED***"
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../merchantauthenticationservice-createserversessionauthenticationtoken.md) | [Back to Overview](../../../test_overview.md)
