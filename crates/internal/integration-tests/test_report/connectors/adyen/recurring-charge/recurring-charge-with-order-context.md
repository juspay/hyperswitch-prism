# Connector `adyen` / Suite `recurring_charge` / Scenario `recurring_charge_with_order_context`

- Service: `RecurringPaymentService/Charge`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"121","message":"Required field 'shopperReference' is not provided.","reason":"Required field 'shopperReference' is not provided."}}
```

**Pre Requisites Executed**

<details>
<summary>1. setup_recurring(setup_recurring) — PASS</summary>

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
  "merchant_recurring_payment_id": "mrpi_5c2cd45a241a4a128a175260",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "5101180000000007"
      },
      "card_exp_month": {
        "value": "03"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "737"
      },
      "card_holder_name": {
        "value": "Ava Wilson"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ethan Johnson",
    "email": {
      "value": "sam.3097@example.com"
    },
    "id": "cust_bef37a6f6d32481bbc473496",
    "phone_number": "+442497565896"
  },
  "return_url": "https://google.com",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "9330 Main Dr"
      },
      "line2": {
        "value": "5799 Sunset Ave"
      },
      "line3": {
        "value": "4894 Market Dr"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66089"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.9864@sandbox.example.com"
      },
      "phone_number": {
        "value": "1676898450"
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
  }
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
content-type: application/grpc
date: Tue, 24 Mar 2026 06:38:38 GMT
x-request-id: setup_recurring_setup_recurring_req

Response contents:
{
  "connectorRecurringPaymentId": "B22H5T8NJGH45G75",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 06:38:35 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "SN5VBLNZK22M5375",
    "set-cookie": "JSESSIONID=BF0D861F725036C42B293EB0AF7D8A81; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-7ed62bd81e815aa1f61a27580c908768-77b784330cfd6722-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "JLCP8FXLCHLF6975"
    }
  },
  "networkTransactionId": "RMEL1O1PQ0324",
  "merchantRecurringPaymentId": "mrpi_5c2cd45a241a4a128a175260",
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "authCode": "020732"
      }
    }
  },
  "capturedAmount": "6000",
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
  -H "x-request-id: recurring_charge_recurring_charge_with_order_context_req" \
  -H "x-connector-request-reference-id: recurring_charge_recurring_charge_with_order_context_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RecurringPaymentService/Charge <<'JSON'
{
  "merchant_charge_id": "mchi_508b1912cbd742e0917c1a0d",
  "connector_recurring_payment_id": {
    "connector_mandate_id": {
      "connector_mandate_id": "JLCP8FXLCHLF6975"
    }
  },
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "merchant_order_id": "gen_887008",
  "webhook_url": "https://example.com/payment/webhook",
  "return_url": "https://google.com",
  "description": "Recurring charge with order context",
  "off_session": true,
  "test_mode": true,
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
x-connector-request-reference-id: recurring_charge_recurring_charge_with_order_context_ref
x-merchant-id: test_merchant
x-request-id: recurring_charge_recurring_charge_with_order_context_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 06:38:40 GMT
x-request-id: recurring_charge_recurring_charge_with_order_context_req

Response contents:
{
  "connectorTransactionId": "WH95N5V2J6HM7L75",
  "error": {
    "issuerDetails": {
      "networkDetails": {}
    },
    "connectorDetails": {
      "code": "121",
      "message": "Required field 'shopperReference' is not provided.",
      "reason": "Required field 'shopperReference' is not provided."
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "cache-control": "no-cache, no-store, private, must-revalidate, max-age=0",
    "content-type": "application/json;charset=UTF-8",
    "date": "Tue, 24 Mar 2026 06:38:39 GMT",
    "expires": "0",
    "pragma": "no-cache",
    "pspreference": "WH95N5V2J6HM7L75",
    "set-cookie": "JSESSIONID=7FAB43A4F6B0AF0D0AFF6B74F8A5B751; Path=/checkout; Secure; HttpOnly",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "traceparent": "00-e6564222daed851a9a5d9874021407f9-872477c55f807b8f-01",
    "transfer-encoding": "chunked",
    "x-content-type-options": "nosniff",
    "x-frame-options": "SAMEORIGIN"
  },
  "merchantChargeId": "WH95N5V2J6HM7L75",
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
