# Connector `nuvei` / Suite `MerchantAuthenticationService/CreateClientAuthenticationToken` / Scenario `Create SDK Session Token | Create SDK Session Google Pay`

- Service: `Unknown`
- Scenario Key: `create_sdk_session_google_pay`
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
  -H "x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_google_pay_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_google_pay_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateClientAuthenticationToken <<'JSON'
{
  "merchant_client_session_id": "gen_606735",
  "payment": {
    "amount": {
      "minor_amount": 12000,
      "currency": "USD"
    },
    "payment_method_type": "GOOGLE_PAY",
    "country_alpha2_code": "US",
    "customer": {
      "id": "cust_35c176f6208f4d1a99ac9db3",
      "name": "Noah Johnson",
      "email": {
        "value": "sam.1322@testmail.io"
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
x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_google_pay_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_google_pay_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 10 Apr 2026 21:21:13 GMT
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_google_pay_req

Response contents:
{
  "sessionData": {
    "connectorSpecific": {
      "nuvei": {
        "sessionToken": ***MASKED***
          "value": "ca633527c93f4e868aede4bc227eec7c0121"
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
