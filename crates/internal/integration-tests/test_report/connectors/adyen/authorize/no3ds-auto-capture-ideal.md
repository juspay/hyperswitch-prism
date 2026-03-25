# Connector `adyen` / Suite `authorize` / Scenario `no3ds_auto_capture_ideal`

- Service: `PaymentService/Authorize`
- PM / PMT: `ideal` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_ideal_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_138e78a948e24fe78b541745",
  "amount": {
    "minor_amount": 6000,
    "currency": "EUR"
  },
  "order_tax_amount": 0,
  "shipping_cost": 0,
  "payment_method": {
    "ideal": {}
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Brown",
    "email": {
      "value": "casey.9235@testmail.io"
    },
    "id": "cust_0cbb5dd1b47b4fc0ac68a756",
    "phone_number": "+914431075773"
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
    "time_zone_offset_minutes": -480,
    "language": "en-US"
  },
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "4179 Market St"
      },
      "line2": {
        "value": "4656 Main Ave"
      },
      "line3": {
        "value": "6755 Sunset Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "16449"
      },
      "country_alpha2_code": "NL",
      "email": {
        "value": "morgan.2834@example.com"
      },
      "phone_number": {
        "value": "4412340808"
      },
      "phone_country_code": "+31"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "9129 Pine Ln"
      },
      "line2": {
        "value": "9113 Oak Ln"
      },
      "line3": {
        "value": "7525 Main Dr"
      },
      "city": {
        "value": "San Francisco"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "20227"
      },
      "country_alpha2_code": "NL",
      "email": {
        "value": "casey.6823@example.com"
      },
      "phone_number": {
        "value": "1147828657"
      },
      "phone_country_code": "+31"
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
  "description": "No3DS auto capture iDEAL payment",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_ideal_ref
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_ideal_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 03:26:14 GMT
x-request-id: authorize_no3ds_auto_capture_ideal_req

Response contents:
{
  "merchantTransactionId": "ZGKPHXX4Z5SK8B75",
  "connectorTransactionId": "ZGKPHXX4Z5SK8B75",
  "status": "AUTHENTICATION_PENDING",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 03:26:14 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "SGHLQRQ2J6HM7L75",
    "set-cookie": "JSESSIONID=44E09C37F83C51C0E2D5F5D3B027F8AC; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-140ab7a05dad8f3f6f156f410430e412-909842f53991f9ec-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
  },
  "redirectionData": {
    "form": {
      "endpoint": "https://checkoutshopper-test.adyen.com/checkoutshopper/checkoutPaymentRedirect?redirectData=X3XtfGC9%21H4sIAAAAAAAA%2F61Va2%2FiOBT9L5Hm0zbgOHbsIFWjkAevQnkESBFS5SQGMg0hTZyqqOp%2FHwfKLDvdnZkPKyVSuD73cc6917wpnbTiOdtyhwmmtN4UlicLXpTJIVNaBr1RwoJlsX2IudJSkpizVLlRooIzwWNLSBsE0FCBrkLkA9SCRktDfwGtBYDE8dc8KXj5Txz%2BjEtiCeiMTZ0Sm1KKTBd7Jsak7biaJ8%2F3vIh2LBNWFB2qrI7Wr8qcHR3Xte%2BHElDwWCaKxJgd9zwTP7hEz5W0Fx9%2BPZnGACYwJS22P0d6U6KqKHgWHWVUdz6VwV6YlKRGAvB%2B80d6%2FLa%2BC2DKN7xOVjvvRfKo6ZQTykxEOUQb%2BRlipBGEpcuhSLZJxtJxmV97zTrdu8l0AvtGd0juSI3MWTrlZZWKn6CrzmDcDQK0wrMBbZ%2BgBX%2BueCnrPOtyjSYaBZgYFBOITKqbV%2Bg4ToTkz9KLrtGOR0%2BHSjQuH5YQfJ%2BfBFYwNeNNpEdqaIShivTYVGnEsIqMMKY8ZBsEJUeCdIgwQZpBXJe2dQtQr40MDG1gWrpDNegZxLE8HUBsWtAhbeB60DEI9AgkQOINgixZ66nSi769TPBtwepqGy%2BXrikGPc0ijyrB%2FV3BuSONG5aW%2FEQyl8OsHjJVJHsuuVyd%2FVtYccxrueyuaw%2Fu5%2F5jxx25056tvNehRFVk8yKV5zsh8rK1bq6bF43K3SHPeaEKKWmDxUeeNaLD%2FtP534aPYZ6eon7d5kl8%2B4sdKXkqF4DH7Xo%2Br2bzI2xNoWCROCviysw1uZpk%2BZRkH%2FOcV2HjBTYCf3JH%2FL6%2FWgyITShsjM3xMNRoPqqs0WRnQ88XHT2dhSh%2Begm%2FLbxqkpflY3Dkz%2BCsw3kbf1aCv4qG3IrGqbRGlq6bQhZbnqsq180T8otufYGefGq0eP0BlhYoX8sYTWxnulxMu8jRMQ6mq%2BnKmU2mXu9rmWxv7U7bsdxez9JdPHiYzfs9bzmY2KNJezxBo641W%2FkLZ7bAhj8fBag%2Ff3CdkT9Y2XbPbZ%2F8Hu6xPe5I9gQGAxL4ltVx7u8ta7jEVn%2FZnwT%2BEA6J6w1sOA5k2sAILOW%2Fes9f2T5P%2BbnR%2Bbmf6%2BYZe%2B00ZgXbc9mhsl6vutW%2FvhDzMnej3eG8j58W%2FeOSa802ybppHvTimRtuu0zGu%2BWyJ0Yz%2FM3r3daN%2Bp9bf6PIfkZJtr3cE%2FVP%2Becir5h6qwBQNQRYSBjAMYvpRt8YGw0bG6QBpAOONKhyTEKGIhpCTde1kKhAU97f378D%2BwN4UacGAAA%3Dwu0xIoNVIKhgyIDgTAC1qPOgxFPCGR49kmp%2Flpo6uBA%3D",
      "method": "HTTP_METHOD_GET",
      "formFields": {
        "redirectData": "X3XtfGC9!H4sIAAAAAAAA/61Va2/iOBT9L5Hm0zbgOHbsIFWjkAevQnkESBFS5SQGMg0hTZyqqOp/HwfKLDvdnZkPKyVSuD73cc6917wpnbTiOdtyhwmmtN4UlicLXpTJIVNaBr1RwoJlsX2IudJSkpizVLlRooIzwWNLSBsE0FCBrkLkA9SCRktDfwGtBYDE8dc8KXj5Txz+jEtiCeiMTZ0Sm1KKTBd7Jsak7biaJ8/3vIh2LBNWFB2qrI7Wr8qcHR3Xte+HElDwWCaKxJgd9zwTP7hEz5W0Fx9+PZnGACYwJS22P0d6U6KqKHgWHWVUdz6VwV6YlKRGAvB+80d6/La+C2DKN7xOVjvvRfKo6ZQTykxEOUQb+RlipBGEpcuhSLZJxtJxmV97zTrdu8l0AvtGd0juSI3MWTrlZZWKn6CrzmDcDQK0wrMBbZ+gBX+ueCnrPOtyjSYaBZgYFBOITKqbV+g4ToTkz9KLrtGOR0+HSjQuH5YQfJ+fBFYwNeNNpEdqaIShivTYVGnEsIqMMKY8ZBsEJUeCdIgwQZpBXJe2dQtQr40MDG1gWrpDNegZxLE8HUBsWtAhbeB60DEI9AgkQOINgixZ66nSi769TPBtwepqGy+XrikGPc0ijyrB/V3BuSONG5aW/EQyl8OsHjJVJHsuuVyd/VtYccxrueyuaw/u5/5jxx25056tvNehRFVk8yKV5zsh8rK1bq6bF43K3SHPeaEKKWmDxUeeNaLD/tP534aPYZ6eon7d5kl8+4sdKXkqF4DH7Xo+r2bzI2xNoWCROCviysw1uZpk+ZRkH/OcV2HjBTYCf3JH/L6/WgyITShsjM3xMNRoPqqs0WRnQ88XHT2dhSh+egm/LbxqkpflY3Dkz+Csw3kbf1aCv4qG3IrGqbRGlq6bQhZbnqsq180T8otufYGefGq0eP0BlhYoX8sYTWxnulxMu8jRMQ6mq+nKmU2mXu9rmWxv7U7bsdxez9JdPHiYzfs9bzmY2KNJezxBo641W/kLZ7bAhj8fBag/f3CdkT9Y2XbPbZ/8Hu6xPe5I9gQGAxL4ltVx7u8ta7jEVn/ZnwT+EA6J6w1sOA5k2sAILOW/es9f2T5P+bnR+bmf6+YZe+00ZgXbc9mhsl6vutW/vhDzMnej3eG8j58W/eOSa802ybppHvTimRtuu0zGu+WyJ0Yz/M3r3daN+p9bf6PIfkZJtr3cE/VP+ecir5h6qwBQNQRYSBjAMYvpRt8YGw0bG6QBpAOONKhyTEKGIhpCTde1kKhAU97f378D+wN4UacGAAA=wu0xIoNVIKhgyIDgTAC1qPOgxFPCGR49kmp/lpo6uBA="
      }
    }
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
