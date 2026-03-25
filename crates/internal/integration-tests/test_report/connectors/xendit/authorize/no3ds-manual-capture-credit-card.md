# Connector `xendit` / Suite `authorize` / Scenario `no3ds_manual_capture_credit_card`

- Service: `PaymentService/Authorize`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_f1adf6e1178847b390ba7210",
  "amount": {
    "minor_amount": 1500000,
    "currency": "IDR"
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
        "value": "Liam Brown"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Emma Johnson",
    "email": {
      "value": "sam.1236@sandbox.example.com"
    },
    "id": "cust_64416033a9b8471bb1bb64c4",
    "phone_number": "+441944127085"
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
        "value": "3188 Oak Ave"
      },
      "line2": {
        "value": "8424 Main Rd"
      },
      "line3": {
        "value": "5714 Sunset Blvd"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "52063"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "sam.9686@example.com"
      },
      "phone_number": {
        "value": "4235332866"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4551 Pine St"
      },
      "line2": {
        "value": "8437 Sunset Dr"
      },
      "line3": {
        "value": "9371 Market Dr"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "43300"
      },
      "country_alpha2_code": "ID",
      "email": {
        "value": "casey.4819@example.com"
      },
      "phone_number": {
        "value": "7472021116"
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
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:36:19 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "a9bdc212-6885-460f-99e7-d3d09641acad",
  "connectorTransactionId": "pr-62beb328-b098-4250-9a64-043bc799e2fe",
  "status": "PENDING",
  "statusCode": 201,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13512a1dea054c-BOM",
    "connection": "keep-alive",
    "content-length": "1689",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 05:36:16 GMT",
    "rate-limit-limit": "60",
    "rate-limit-remaining": "56",
    "rate-limit-reset": "51.964",
    "request-id": "69c222ce00000000599d59afe7e608a5",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=odGC61ct38LLPyK.tyMXuTfBIESGCrldXyqyiVswhP8-1774330574.413572-1.0.1.1-MaaILj5h9R62s7oTtvWtV9kzmrGWBFPRcv2Zwh4AMhRKsjn6Io6.4dBPylL952WyQ72xptUwpEP8nbvQt1hiRSwdwX8RkjZBQ8Q87fvZjahMfvHeUbMBHgfO74sBGQvE; HttpOnly; Secure; Path=/; Domain=xendit.co; Expires=Tue, 24 Mar 2026 06:06:16 GMT",
    "vary": "Origin",
    "x-envoy-upstream-service-time": "1586"
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
