# Connector `payload` / Suite `recurring_charge` / Scenario `recurring_charge`

- Service: `RecurringPaymentService/Charge`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. setup_recurring(setup_recurring) — FAIL</summary>

**Dependency Error**

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:23:54 GMT
x-request-id: setup_recurring_setup_recurring_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Setup mandate with non zero amount flow not supported by Payload connector
```

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_f7f352aab3444d32a2a29e7181364f99",
  "amount": {
    "minor_amount": 6000,
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
        "value": "Liam Brown"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "jordan.7011@sandbox.example.com"
    },
    "id": "cust_2934370a615e4429a763c0dd051f1b3e",
    "phone_number": "+13764904570"
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
        "value": "Ava"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "1870 Sunset Rd"
      },
      "line2": {
        "value": "7198 Main Ave"
      },
      "line3": {
        "value": "6601 Lake Blvd"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "75442"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.6204@testmail.io"
      },
      "phone_number": {
        "value": "6918800393"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
  },
  "setup_future_usage": "OFF_SESSION"
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Setup a recurring payment instruction for future payments/ debits. This could be
// for SaaS subscriptions, monthly bill payments, insurance payments and similar use cases.
rpc SetupRecurring ( .types.PaymentServiceSetupRecurringRequest ) returns ( .types.PaymentServiceSetupRecurringResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
(empty)

Response trailers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:23:54 GMT
x-request-id: setup_recurring_setup_recurring_req
Sent 1 request and received 0 responses

ERROR:
  Code: InvalidArgument
  Message: Setup mandate with non zero amount flow not supported by Payload connector
```

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: recurring_charge_recurring_charge_req" \
  -H "x-connector-request-reference-id: recurring_charge_recurring_charge_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RecurringPaymentService/Charge <<'JSON'
{
  "merchant_charge_id": "mchi_bee3e2d76bd1433bb73f0c15fc54d1b3",
  "connector_recurring_payment_id": {
    "connector_mandate_id": {
      "connector_mandate_id": "cmi_abbea48ee2b04377965405fe1e87b797"
    }
  },
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_customer_id": "",
  "customer": {}
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Charge using an existing stored recurring payment instruction. Processes repeat payments for
// subscriptions or recurring billing without collecting payment details.
rpc Charge ( .types.RecurringPaymentServiceChargeRequest ) returns ( .types.RecurringPaymentServiceChargeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: recurring_charge_recurring_charge_ref
x-merchant-id: test_merchant
x-request-id: recurring_charge_recurring_charge_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 16:23:55 GMT
x-request-id: recurring_charge_recurring_charge_req

Response contents:
{
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "InvalidAttributes",
      "message": "payment_method_id",
      "reason": "{\"payment_method_id\":\"Invalid\"}"
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, must-revalidate",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e0ec889fbd33e33-BOM",
    "connection": "keep-alive",
    "content-length": "134",
    "content-type": "application/json",
    "date": "Mon, 23 Mar 2026 16:23:55 GMT",
    "pragma": "no-cache",
    "server": "cloudflare",
    "strict-transport-security": "max-age=31536000; includeSubDomains"
  },
  "state": {
    "connectorCustomerId": ""
  },
  "rawConnectorRequest": "***MASKED***"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../recurring-charge.md) | [Back to Overview](../../../test_overview.md)
