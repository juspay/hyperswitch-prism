# Connector `bluesnap` / Suite `authorize` / Scenario `no3ds_fail_payment`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to exist
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_fail_payment_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_fail_payment_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_cbf6e4b9ece54340ac575630",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4000000000000002"
      },
      "card_exp_month": {
        "value": "01"
      },
      "card_exp_year": {
        "value": "35"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Mia Wilson"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ava Miller",
    "email": {
      "value": "morgan.6352@testmail.io"
    },
    "id": "cust_3315b39e066f43bbba503d84",
    "phone_number": "+17629151847"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "6007 Oak Blvd"
      },
      "line2": {
        "value": "5459 Market Blvd"
      },
      "line3": {
        "value": "1615 Market St"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "47895"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.1935@sandbox.example.com"
      },
      "phone_number": {
        "value": "6772636490"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6224 Sunset Ln"
      },
      "line2": {
        "value": "4917 Pine Dr"
      },
      "line3": {
        "value": "7522 Pine Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "85970"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.8620@testmail.io"
      },
      "phone_number": {
        "value": "2793549497"
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
  "description": "No3DS fail payment flow",
  "payment_channel": "ECOMMERCE",
  "test_mode": true,
  "locale": "en-US"
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
x-connector-request-reference-id: authorize_no3ds_fail_payment_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_fail_payment_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:32:09 GMT
x-request-id: authorize_no3ds_fail_payment_req

Response contents:
{
  "merchantTransactionId": "1087579056",
  "connectorTransactionId": "1087579056",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0f8454ff955644-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 18:32:09 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=.sx7heWWrS6vP_uoS_kLwskB_a9CdRK5lqmSj1w7pLU-1774290729-1.0.1.1-Q3b_k3N4pYK3SuQlttu2fw530yjuy6DYskatsOBv3QZ3_CCmfgtHKF8j0ueVP_yYWbbByUlUsIKegZ08uYoQyCqFrYF1FPHaERPLxAS_2ZY; path=/; expires=Mon, 23-Mar-26 19:02:09 GMT; domain=.bluesnap.com; HttpOnly; Secure",
    "strict-transport-security": "max-age=31536000; includeSubDomains; preload",
    "transfer-encoding": "chunked",
    "vary": "Accept-Encoding"
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


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
