# Connector `rapyd` / Suite `MerchantAuthenticationService/CreateClientAuthenticationToken` / Scenario `Create SDK Session Token | Create SDK Session With Taxes`

- Service: `Unknown`
- Scenario Key: `create_sdk_session_with_taxes`
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
  -H "x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_with_taxes_req" \
  -H "x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_with_taxes_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:8000 types.MerchantAuthenticationService/CreateClientAuthenticationToken <<'JSON'
{
  "merchant_client_session_id": "gen_742549",
  "payment": {
    "amount": {
      "minor_amount": 15000,
      "currency": "USD"
    },
    "order_tax_amount": 1200,
    "shipping_cost": 500,
    "payment_method_type": "APPLE_PAY",
    "country_alpha2_code": "US",
    "customer": {
      "id": "cust_28310d16d81a45408992265a",
      "name": "Ethan Taylor",
      "email": {
        "value": "jordan.4805@testmail.io"
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
x-connector-request-reference-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_with_taxes_ref
x-merchant-id: test_merchant
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_with_taxes_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 13 Apr 2026 16:25:28 GMT
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_with_taxes_req

Response contents:
{
  "sessionData": {
    "connectorSpecific": {
      "rapyd": {
        "checkoutId": "checkout_ed0e831a70c59af1d572b4caea611fbf",
        "redirectUrl": "https://sandboxcheckout.rapyd.net/?token=checkout_ed0e831a70c59af1d572b4caea611fbf"
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
