# Connector `stripe` / Suite `MerchantAuthenticationService/CreateClientAuthenticationToken` / Scenario `Create SDK Session Token | Create SDK Session Apple Pay`

- Service: `Unknown`
- Scenario Key: `create_sdk_session_apple_pay`
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
  -H "x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_apple_pay_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_apple_pay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateClientAuthenticationToken <<'JSON'
{
  "merchant_client_session_id": "gen_789855",
  "payment": {
    "amount": {
      "minor_amount": 10000,
      "currency": "USD"
    },
    "payment_method_type": "APPLE_PAY",
    "country_alpha2_code": "US",
    "customer": {
      "id": "cust_cf6ac3369ba54514bbf97f2e",
      "name": "Ethan Taylor",
      "email": {
        "value": "riley.3751@testmail.io"
      }
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
// Initialize client-facing SDK sessions for wallets, device fingerprinting,
// etc. Returns structured data the client SDK needs to render
// payment/verification UI.
rpc CreateClientAuthenticationToken ( .types.MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest ) returns ( .types.MerchantAuthenticationServiceCreateClientAuthenticationTokenResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_apple_pay_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_apple_pay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Sat, 11 Apr 2026 19:39:49 GMT
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_apple_pay_req

Response contents:
{
  "sessionData": {
    "connectorSpecific": {
      "stripe": {
        "clientSecret": ***MASKED***
          "value": "pi_3TL7NhD5R7gDAGff1qHGrQGN_secret_8vNTzrJYtOomHntMbV1Lqcqiw"
        }
      }
    }
  },
  "statusCode": 200,
  "rawConnectorResponse": "***MASKED***",
  "rawConnectorRequest": "***MASKED***"


Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../merchantauthenticationservice-createclientauthenticationtoken.md) | [Back to Overview](../../../test_overview.md)
