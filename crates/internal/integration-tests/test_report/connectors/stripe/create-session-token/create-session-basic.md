# Connector `stripe` / Suite `create_session_token` / Scenario `create_session_basic`

- Service: `Unknown`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Create session token for payment processing. Maintains session state
// across multiple payment operations for improved security and tracking.
rpc CreateSessionToken ( .types.MerchantAuthenticationServiceCreateSessionTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateSessionTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_session_token_create_session_basic_ref
x-merchant-id: test_merchant
x-request-id: create_session_token_create_session_basic_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:50:32 GMT
x-request-id: create_session_token_create_session_basic_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Session Token creation failed: InternalServerError
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_session_token_create_session_basic_req" \
  -H "x-connector-request-reference-id: create_session_token_create_session_basic_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.MerchantAuthenticationService/CreateSessionToken <<'JSON'
{
  "merchant_session_id": "gen_689446",
  "amount": {
    "minor_amount": 10000,
    "currency": "USD"
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
// Create session token for payment processing. Maintains session state
// across multiple payment operations for improved security and tracking.
rpc CreateSessionToken ( .types.MerchantAuthenticationServiceCreateSessionTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateSessionTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_session_token_create_session_basic_ref
x-merchant-id: test_merchant
x-request-id: create_session_token_create_session_basic_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 08:50:32 GMT
x-request-id: create_session_token_create_session_basic_req
Sent 1 request and received 0 responses

ERROR:
  Code: Internal
  Message: Session Token creation failed: InternalServerError
```

</details>


[Back to Connector Suite](../create-session-token.md) | [Back to Overview](../../../test_overview.md)
