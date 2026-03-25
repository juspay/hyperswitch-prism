# Connector `bluesnap` / Suite `authorize` / Scenario `no3ds_auto_capture_bancontact`

- Service: `PaymentService/Authorize`
- PM / PMT: `bancontact_card` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_bancontact_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_f415e441c6974dcdaa0b47da",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "bancontact_card": {
      "card_number": ***MASKED***
        "value": "4111111111111111"
      },
      "card_exp_month": {
        "value": "08"
      },
      "card_exp_year": {
        "value": "30"
      },
      "card_holder_name": {
        "value": "Mia Smith"
      }
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Emma Smith",
    "email": {
      "value": "riley.3549@testmail.io"
    },
    "id": "cust_c1f05546b0f24af29a0c7d58",
    "phone_number": "+912144731744"
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
        "value": "Noah"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "5805 Market St"
      },
      "line2": {
        "value": "5333 Market Dr"
      },
      "line3": {
        "value": "482 Lake Ave"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "52294"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "sam.6291@example.com"
      },
      "phone_number": {
        "value": "3462227737"
      },
      "phone_country_code": "+32"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9477 Market Dr"
      },
      "line2": {
        "value": "3287 Market Ave"
      },
      "line3": {
        "value": "9457 Sunset Blvd"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "VLG"
      },
      "zip_code": {
        "value": "52197"
      },
      "country_alpha2_code": "BE",
      "email": {
        "value": "riley.4680@example.com"
      },
      "phone_number": {
        "value": "2850800632"
      },
      "phone_country_code": "+32"
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
  "description": "No3DS auto capture Bancontact payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_bancontact_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_bancontact_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:31:59 GMT
x-request-id: authorize_no3ds_auto_capture_bancontact_req

Response contents:
{
  "status": "FAILURE",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "NOT_IMPLEMENTED",
      "message": "This step has not been implemented for: Selected payment method is not supported"
    }
  },
  "statusCode": 501
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
