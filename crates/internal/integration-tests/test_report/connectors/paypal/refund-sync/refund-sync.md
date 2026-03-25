# Connector `paypal` / Suite `refund_sync` / Scenario `refund_sync`

- Service: `RefundService/Get`
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
date: Mon, 23 Mar 2026 16:27:06 GMT
x-request-id: create_access_token_create_access_token_req

Response contents:
{
  "accessToken": ***MASKED***
    "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
  },
  "expiresInSeconds": "32279",
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
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_6a2a1416f6074c699668df8e935034c9",
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
        "value": "Noah Smith"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "sam.4986@sandbox.example.com"
    },
    "id": "cust_028d2d90073849aa9de52baafe7141f4",
    "phone_number": "+917661378020"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32279"
    }
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "589 Lake Rd"
      },
      "line2": {
        "value": "6872 Lake Ln"
      },
      "line3": {
        "value": "1179 Lake Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91666"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.9005@sandbox.example.com"
      },
      "phone_number": {
        "value": "7315118978"
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
        "value": "4850 Oak Rd"
      },
      "line2": {
        "value": "4672 Oak St"
      },
      "line3": {
        "value": "1288 Market Rd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "38493"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "jordan.2264@testmail.io"
      },
      "phone_number": {
        "value": "2670894755"
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
<summary>Show Dependency Response (masked)</summary>

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
date: Mon, 23 Mar 2026 16:27:09 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "mti_6a2a1416f6074c699668df8e935034c9",
  "connectorTransactionId": "5C4822498X0990158",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2396",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:27:09 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f139537281be0",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f139537281be0-870e475d72a1d529-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-backend-info": "v=1;name=2k1u3gOGb2cebCyZJujTUN--F_ccg18_wju_origin_api_m_2_sandbox_paypal_com;ip=34.106.111.220;port=443;ssl=1;max=200;mr=0;ka_ns=0;ml_ns=0;tka_s=300;tki_s=10;tkp=3;host=api-m.sandbox.paypal.com;min_tls=;max_tls=;sni=edge.sandbox.paypal.com;cert_host=edge.sandbox.paypal.com;ciphers=;check_cert=1;no_reneg=1;to_ns=1000000000;fbto_ns=59000000000;bbto_ns=10000000000;fto_ns=0",
    "x-cache": "MISS, MISS, MISS",
    "x-cache-hits": "0, 0, 0",
    "x-served-by": "cache-sin-wsss1830046-SIN, cache-sin-wsss1830046-SIN, cache-bom-vanm7210065-BOM",
    "x-timer": "S1774283227.594005,VS0,VE2950"
  },
  "state": {
    "accessToken": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expiresInSeconds": "32279"
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
    "value": "{\"authorize_id\":null,\"capture_id\":\"4DK9219252505005W\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>3. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_f97d3dc06fb34d5e86704a07ec06e883",
  "connector_transaction_id": "5C4822498X0990158",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32279"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"4DK9219252505005W\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_full_amount_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:27:12 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "connectorRefundId": "8BU47635W1861942D",
  "status": "REFUND_SUCCESS",
  "statusCode": 201,
  "responseHeaders": {
    "accept-ranges": "bytes",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-length": "710",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:27:12 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f402673952fff",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f402673952fff-15ca2ba1e1f12db7-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-backend-info": "v=1;name=2k1u3gOGb2cebCyZJujTUN--F_ccg18_wju_origin_api_m_1_sandbox_paypal_com;ip=34.106.238.133;port=443;ssl=1;max=200;mr=0;ka_ns=0;ml_ns=0;tka_s=300;tki_s=10;tkp=3;host=api-m.sandbox.paypal.com;min_tls=;max_tls=;sni=edge.sandbox.paypal.com;cert_host=edge.sandbox.paypal.com;ciphers=;check_cert=1;no_reneg=1;to_ns=1000000000;fbto_ns=59000000000;bbto_ns=10000000000;fto_ns=0",
    "x-cache": "MISS, MISS, MISS",
    "x-cache-hits": "0, 0, 0",
    "x-served-by": "cache-sin-wsat1880065-SIN, cache-sin-wsat1880065-SIN, cache-bom-vanm7210065-BOM",
    "x-timer": "S1774283230.738528,VS0,VE3100"
  },
  "connectorTransactionId": "5C4822498X0990158",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
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
  -H "x-request-id: refund_sync_refund_sync_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "5C4822498X0990158",
  "refund_id": "8BU47635W1861942D",
  "state": {
    "access_token": ***MASKED***
      "token": ***MASKED***
        "value": "A21AAJZjSsDmCuE5WX3HtseBQeY2EE9Hnb7dx0oLx2msatk8EQXES2DtKeeLgw5x-Lt04D71PcF7COTmnrPA2Hd52N5PZJVlQ"
      },
      "expires_in_seconds": "32279"
    }
  },
  "connector_feature_data": {
    "value": "{\"authorize_id\":null,\"capture_id\":\"4DK9219252505005W\",\"incremental_authorization_id\":null,\"psync_flow\":\"CAPTURE\",\"next_action\":null,\"order_id\":null}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_sync_refund_sync_ref
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:27:14 GMT
x-request-id: refund_sync_refund_sync_req

Response contents:
{
  "merchantRefundId": "8BU47635W1861942D",
  "connectorRefundId": "8BU47635W1861942D",
  "status": "REFUND_SUCCESS",
  "statusCode": 200,
  "responseHeaders": {
    "accept-ranges": "none",
    "access-control-expose-headers": "Server-Timing",
    "cache-control": "max-age=0, no-cache, no-store, must-revalidate",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:27:14 GMT",
    "edge-control": "max-age=0",
    "http_x_pp_az_locator": "ccg18.slc",
    "paypal-debug-id": "f343979585a6c",
    "server": "nginx",
    "server-timing": "traceparent;desc=\"00-0000000000000000000f343979585a6c-37ec9aa3059157f9-01\"",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding",
    "via": "1.1 varnish, 1.1 varnish",
    "x-backend-info": "v=1;name=2k1u3gOGb2cebCyZJujTUN--F_ccg18_wju_origin_api_m_1_sandbox_paypal_com;ip=34.106.238.133;port=443;ssl=1;max=200;mr=0;ka_ns=0;ml_ns=0;tka_s=300;tki_s=10;tkp=3;host=api-m.sandbox.paypal.com;min_tls=;max_tls=;sni=edge.sandbox.paypal.com;cert_host=edge.sandbox.paypal.com;ciphers=;check_cert=1;no_reneg=1;to_ns=1000000000;fbto_ns=59000000000;bbto_ns=10000000000;fto_ns=0",
    "x-cache": "MISS, MISS, MISS",
    "x-cache-hits": "0, 0, 0",
    "x-served-by": "cache-sin-wsap440023-SIN, cache-sin-wsap440023-SIN, cache-bom-vanm7210065-BOM",
    "x-timer": "S1774283233.996423,VS0,VE1110"
  },
  "connectorTransactionId": "5C4822498X0990158",
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
