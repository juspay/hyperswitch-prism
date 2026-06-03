# Connector `nuvei` / Suite `MerchantAuthenticationService/CreateClientAuthenticationToken` / Scenario `Create SDK Session Token | Create SDK Session With Taxes`

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
  "merchant_client_session_id": "gen_121669",
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
      "id": "cust_2a5afecea4f448a098a9ccd5",
      "name": "Emma Johnson",
      "email": {
        "value": "riley.9500@example.com"
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
date: Fri, 10 Apr 2026 21:21:13 GMT
x-request-id: MerchantAuthenticationService/CreateClientAuthenticationToken_create_sdk_session_with_taxes_req

Response contents:
{
  "sessionData": {
    "connectorSpecific": {
      "nuvei": {
        "sessionToken": ***MASKED***
          "value": "538fd53a43924beb8d05b2e3a9bec37b0121"
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
