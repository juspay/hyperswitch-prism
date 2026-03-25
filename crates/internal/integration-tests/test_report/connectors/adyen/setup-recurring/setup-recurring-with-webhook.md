# Connector `adyen` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

- Service: `PaymentService/SetupRecurring`
- PM / PMT: `card` / `credit`
- Result: `FAIL`

**Error**

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_with_webhook_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:23:17 GMT
x-request-id: setup_recurring_setup_recurring_with_webhook_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Missing required field: browser_info.language
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_with_webhook_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_d89a85941b9747869ef39f4d",
  "amount": {
    "minor_amount": 4500,
    "currency": "USD"
  },
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
        "value": "Liam Wilson"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ava Brown",
    "email": {
      "value": "morgan.5229@example.com"
    },
    "id": "cust_41441b4fde134706b609f415",
    "phone_number": "+16916600801"
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
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "4256 Oak Rd"
      },
      "line2": {
        "value": "2089 Oak Dr"
      },
      "line3": {
        "value": "4448 Market St"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66488"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.5746@testmail.io"
      },
      "phone_number": {
        "value": "6347226049"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION",
  "return_url": "https://example.com/payment/return",
  "webhook_url": "https://example.com/payment/webhook"
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_with_webhook_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:23:17 GMT
x-request-id: setup_recurring_setup_recurring_with_webhook_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Missing required field: browser_info.language
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
