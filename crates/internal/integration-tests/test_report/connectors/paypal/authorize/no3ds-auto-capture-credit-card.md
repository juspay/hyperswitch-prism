# Connector `paypal` / Suite `authorize` / Scenario `no3ds_auto_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

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
date: Mon, 23 Mar 2026 16:25:06 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
  },
  "expiresInSeconds": "32399",
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

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_5b8d26fa9d504aafa8c1ac29de6522bf",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_cvc": ***MASKED***
        "value": "999"
      },
      "card_holder_name": {
        "value": "Mia Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Liam Brown",
    "email": {
      "value": "alex.9861@testmail.io"
    },
    "id": "cust_11f7300c0c8e4088934d0e3849f3b329",
    "phone_number": "+445643489431"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32399"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "2576 Sunset Ln"
      },
      "line2": {
        "value": "5740 Oak Blvd"
      },
      "line3": {
        "value": "2697 Lake St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "48425"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.4968@testmail.io"
      },
      "phone_number": {
        "value": "1477732999"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8520 Oak Blvd"
      },
      "line2": {
        "value": "1045 Lake Rd"
      },
      "line3": {
        "value": "3089 Oak Ln"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "43243"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.9574@testmail.io"
      },
      "phone_number": {
        "value": "2959655261"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook",
  "complete_authorize_url": "https://example.com/payment/complete",
  "order_category": "physical",
  "setup_future_usage": "ON_SESSION",
  "off_session": false,
  "description": "No3DS auto capture card payment (credit)",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US",
  "browser_info": {
    "ip_address": "127.0.0.1",
    "accept_header": "application/json",
    "user_agent": "Mozilla/5.0 (integration-tests)",
    "accept_language": "en-US",
    "color_depth": 24,
    "screen_height": 1080,
    "screen_width": 1920,
    "java_enabled": false,
    "java_script_enabled": true,
    "time_zone_offset_minutes": -480
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:25:11 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_5b8d26fa9d504aafa8c1ac29de6522bf",
  "connectorTransactionId": "4J666465S6867354S",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2393",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:25:11 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f33691212de47",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f33691212de47-19ef52f2986a7c9e-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-backend-info": "v=1;name=2k1u3gOGb2cebCyZJujTUN--F_ccg18_wju_origin_api_m_1_sandbox_paypal_com;ip=34.106.238.133;port=443;ssl=1;max=200;mr=0;ka_ns=0;ml_ns=0;tka_s=300;tki_s=10;tkp=3;host=api-m.sandbox.paypal.com;min_tls=;max_tls=;sni=edge.sandbox.paypal.com;cert_host=edge.sandbox.paypal.com;ciphers=;check_cert=1;no_reneg=1;to_ns=1000000000;fbto_ns=59000000000;bbto_ns=10000000000;fto_ns=0",
    "x-cache": "MISS, MISS, MISS",
    "x-cache-hits": "0, 0, 0",
    "x-served-by": "cache-sin-wsat1880058-SIN, cache-sin-wsat1880058-SIN, cache-bom-vanm7210065-BOM",
    "x-timer": "S1774283109.819628,VS0,VE2895"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expiresInSeconds": "32399"
    }
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "mandateReference": {
    "connectorMandateId": {}
  },
  "connectorFeatureData": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"1SV55566K66726500\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
