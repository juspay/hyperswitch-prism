# Connector `paypal` / Suite `authorize` / Scenario `Google Pay (Encrypted Token) | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_google_pay_encrypted`
- PM / PMT: `google_pay` / `CARD`
- Result: `FAIL`

**Error**

```text
grpcurl execution failed: browser automation engine directory does not exist: '/Users/amitsingh.tanwar/Documents/connector-service/connector-service/crates/internal/integration-tests/../../browser-automation-engine'
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
date: Wed, 25 Mar 2026 12:19:39 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AAKQKKJImq223veORho2PTdsoqsgEdO-3srIbJdBewAEX0VwAVHlPt3p6tpZv960hyVTR3aTOP3oQcwfdA1z_cjDo5OcJA"
  },
  "expiresInSeconds": "32400",
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

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)