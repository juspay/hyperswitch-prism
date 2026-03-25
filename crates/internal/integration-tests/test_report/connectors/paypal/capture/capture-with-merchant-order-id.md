# Connector `paypal` / Suite `capture` / Scenario `capture_with_merchant_order_id`

- Service: `PaymentService/Capture`
- PM / PMT: `-` / `-`
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
date: Mon, 23 Mar 2026 16:26:10 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
  },
  "expiresInSeconds": "32335",
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
<summary>2. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_d0aaaa960e7f44cb91316fa5a7065613",
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
        "value": "Mia Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ava Wilson",
    "email": {
      "value": "jordan.7224@testmail.io"
    },
    "id": "cust_5aa918a577d244d5a2e5b84694e9cbe0",
    "phone_number": "+448234680022"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32335"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "2980 Pine Dr"
      },
      "line2": {
        "value": "6285 Main Dr"
      },
      "line3": {
        "value": "437 Lake Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "51661"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2448@example.com"
      },
      "phone_number": {
        "value": "8053572736"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7166 Oak Ave"
      },
      "line2": {
        "value": "6184 Market Dr"
      },
      "line3": {
        "value": "9071 Main Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "42116"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.7588@sandbox.example.com"
      },
      "phone_number": {
        "value": "5575200850"
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
  "description": "No3DS manual capture card payment (credit)",
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
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:26:13 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_d0aaaa960e7f44cb91316fa5a7065613",
  "connectorTransactionId": "6MA54187JP7207335",
  "status": "AUTHORIZED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2521",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:26:13 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f6274447a653f",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f6274447a653f-dae1ab4c01f637da-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-backend-info": "v=1;name=2k1u3gOGb2cebCyZJujTUN--F_ccg18_wju_origin_api_m_2_sandbox_paypal_com;ip=34.106.111.220;port=443;ssl=1;max=200;mr=0;ka_ns=0;ml_ns=0;tka_s=300;tki_s=10;tkp=3;host=api-m.sandbox.paypal.com;min_tls=;max_tls=;sni=edge.sandbox.paypal.com;cert_host=edge.sandbox.paypal.com;ciphers=;check_cert=1;no_reneg=1;to_ns=1000000000;fbto_ns=59000000000;bbto_ns=10000000000;fto_ns=0",
    "x-cache": "MISS, MISS, MISS",
    "x-cache-hits": "0, 0, 0",
    "x-served-by": "cache-sin-wsss1830045-SIN, cache-sin-wsss1830092-SIN, cache-bom-vanm7210065-BOM",
    "x-timer": "S1774283171.765038,VS0,VE2946"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expiresInSeconds": "32335"
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
    "value": "{\"authorize_id\":\"97E78685PU6895947\",\"capture_id\":null,\"incremental_authorization_id\":\"97E78685PU6895947\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
  }
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
  -H "x-request-id: capture_capture_with_merchant_order_id_req" \
  -H "x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Capture <<'JSON'
{
  "connector_transaction_id": "6MA54187JP7207335",
  "amount_to_capture": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_capture_id": "mci_040e1cfd4ad645888ea0dab561a3d1cc",
  "merchant_order_id": "gen_169758",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32335"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":\"97E78685PU6895947\",\"capture_id\":null,\"incremental_authorization_id\":\"97E78685PU6895947\",\"psync_flow\":\"AUTHORIZE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Finalize an authorized payment transaction. Transfers reserved funds from
// customer to merchant account, completing the payment lifecycle.
rpc Capture ( .types.PaymentServiceCaptureRequest ) returns ( .types.PaymentServiceCaptureResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: capture_capture_with_merchant_order_id_ref
x-merchant-id: test_merchant
x-request-id: capture_capture_with_merchant_order_id_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:26:16 GMT
x-request-id: capture_capture_with_merchant_order_id_req

Response contents:
{
  "connectorTransactionId": "6MA54187JP7207335",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "925",
    "content-type": "application/json;charset=UTF-8",
    "date": "Mon, 23 Mar 2026 16:26:16 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f398078925c0c",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f398078925c0c-c3b1be9535691609-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-backend-info": "v=1;name=2k1u3gOGb2cebCyZJujTUN--F_ccg18_wju_origin_api_m_2_sandbox_paypal_com;ip=34.106.111.220;port=443;ssl=1;max=200;mr=0;ka_ns=0;ml_ns=0;tka_s=300;tki_s=10;tkp=3;host=api-m.sandbox.paypal.com;min_tls=;max_tls=;sni=edge.sandbox.paypal.com;cert_host=edge.sandbox.paypal.com;ciphers=;check_cert=1;no_reneg=1;to_ns=1000000000;fbto_ns=59000000000;bbto_ns=10000000000;fto_ns=0",
    "x-cache": "MISS, MISS, MISS",
    "x-cache-hits": "0, 0, 0",
    "x-served-by": "cache-sin-wsat1880040-SIN, cache-sin-wsat1880040-SIN, cache-bom-vanm7210065-BOM",
    "x-timer": "S1774283174.975843,VS0,VE2085"
  },
  "merchantCaptureId": "mti_d0aaaa960e7f44cb91316fa5a7065613",
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expiresInSeconds": "32335"
    }
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "capturedAmount": "6000",
  "connectorFeatureData": {
    "value": "{\"authorize_id\":\"97E78685PU6895947\",\"capture_id\":\"5GW52551CS267412V\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../capture.md) | [Back to Overview](../../../test_overview.md)
