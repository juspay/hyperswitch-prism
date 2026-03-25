# Connector `fiserv` / Suite `refund` / Scenario `refund_with_reason`

- Service: `PaymentService/Refund`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

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
  "merchant_transaction_id": "mti_79703ee5a19a45de8b95e09b",
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
        "value": "Noah Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Mia Miller",
    "email": {
      "value": "sam.7136@sandbox.example.com"
    },
    "id": "cust_3a5255c161744ab9b509eb89",
    "phone_number": "+17250152663"
  },
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
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "7441 Main Blvd"
      },
      "line2": {
        "value": "5641 Sunset Blvd"
      },
      "line3": {
        "value": "5465 Pine Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "18173"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.1646@testmail.io"
      },
      "phone_number": {
        "value": "8816527451"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9075 Main Blvd"
      },
      "line2": {
        "value": "1649 Main Blvd"
      },
      "line3": {
        "value": "8137 Sunset Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41769"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1763@testmail.io"
      },
      "phone_number": {
        "value": "8690816773"
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
  "locale": "en-US"
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
date: Tue, 24 Mar 2026 07:42:05 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "CHG01b8d015abae61363ce068cc437d2e356d",
  "connectorTransactionId": "576af1268634403ba6f4a535e82d1474",
  "status": "CHARGED",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "576af1268634403ba6f4a535e82d1474",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "3482",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:42:05 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338124; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:42:04 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338125107",
    "targetserversentstarttimestamp": "1774338124682",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "c66354fd-59e1-487a-8751-3ab19d6c96157832.1",
    "x-vcap-request-id": "576af126-8634-403b-a6f4-a535e82d1474",
    "x-xss-protection": "1; mode=block"
  },
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
  -H "x-request-id: refund_refund_with_reason_req" \
  -H "x-connector-request-reference-id: refund_refund_with_reason_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_5330adaf0ad745e5bbf49080",
  "connector_transaction_id": "576af1268634403ba6f4a535e82d1474",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "reason": "customer_requested"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: refund_refund_with_reason_ref
x-merchant-id: test_merchant
x-request-id: refund_refund_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:42:06 GMT
x-request-id: refund_refund_with_reason_req

Response contents:
{
  "connectorRefundId": "7fc4aafe7407466c8f165d54bd8033c3",
  "status": "REFUND_SUCCESS",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-headers": "api-key,auth-token-type,authorization,client-request-id,content-type,message-digest,timestamp,x-integration,x-integration-merchant-id,x-integration-origin,x-integration-terminal-id,x-integration-version,x-deploymentrouteto",
    "access-control-allow-origin": "",
    "access-control-max-age": "86400",
    "apitraceid": "7fc4aafe7407466c8f165d54bd8033c3",
    "cache-control": "no-store, no-cache, must-revalidate",
    "connection": "keep-alive",
    "content-length": "2869",
    "content-security-policy": "default-src 'none'; frame-ancestors 'self'; script-src 'unsafe-inline' 'self' *.googleapis.com *.klarna.com *.masterpass.com *.mastercard.com *.newrelic.com *.npci.org.in *.nr-data.net *.google-analytics.com *.google.com *.getsitecontrol.com *.gstatic.com *.kxcdn.com 'strict-dynamic' 'nonce-6f62fa22a79de4c553d2bbde' 'unsafe-eval' 'unsafe-inline'; connect-src 'self'; img-src 'self'; style-src 'self'; base-uri 'self';",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:42:06 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "rdwr_response": "allowed",
    "referrer-policy": "no-referrer",
    "response-category": "processed",
    "set-cookie": "__uzmd=1774338125; HttpOnly; path=/; Expires=Tue, 22-Sep-26 07:42:05 GMT; Max-Age=15724800; SameSite=Lax",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "targetserverreceivedendtimestamp": "1774338126407",
    "targetserversentstarttimestamp": "1774338125985",
    "transactionprocessedin": "chandler",
    "x-content-type-options": "nosniff",
    "x-frame-options": "DENY",
    "x-request-id": "c66354fd-59e1-487a-8751-3ab19d6c96157834.1",
    "x-vcap-request-id": "7fc4aafe-7407-466c-8f16-5d54bd8033c3",
    "x-xss-protection": "1; mode=block"
  },
  "connectorTransactionId": "576af1268634403ba6f4a535e82d1474",
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


[Back to Connector Suite](../refund.md) | [Back to Overview](../../../test_overview.md)
