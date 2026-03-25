# Connector `authorizedotnet` / Suite `recurring_charge` / Scenario `recurring_charge_low_amount`

- Service: `RecurringPaymentService/Charge`
- PM / PMT: `-` / `-`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_f2072b65c74f415cb567fcc3",
  "customer_name": "Emma Miller",
  "email": {
    "value": "jordan.5923@testmail.io"
  },
  "phone_number": "+444552744265",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "47 Market Ln"
      },
      "line2": {
        "value": "9994 Market Blvd"
      },
      "line3": {
        "value": "1134 Lake Dr"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "26504"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.8047@testmail.io"
      },
      "phone_number": {
        "value": "5760877667"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "2976 Sunset Dr"
      },
      "line2": {
        "value": "8844 Market St"
      },
      "line3": {
        "value": "9675 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66853"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2253@example.com"
      },
      "phone_number": {
        "value": "3826965744"
      },
      "phone_country_code": "+91"
    }
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Create customer record in the payment processor system. Stores customer details
// for future payment operations without re-sending personal information.
rpc Create ( .types.CustomerServiceCreateRequest ) returns ( .types.CustomerServiceCreateResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: create_customer_create_customer_ref
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:28:03 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "525968128",
  "connectorCustomerId": "525968128",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "232",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:28:03 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "f7f98837-27d3-43b1-8d07-b9e5c1850eb3-7572-12608428"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. setup_recurring(setup_recurring) — PASS</summary>

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
  "merchant_recurring_payment_id": "mrpi_234687504bd74d6aa37b6f3d",
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
        "value": "Ava Smith"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Noah Wilson",
    "email": {
      "value": "jordan.2476@sandbox.example.com"
    },
    "id": "cust_c5eed054483e453497f501f2",
    "phone_number": "+447389655321",
    "connector_customer_id": "525968128"
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
        "value": "Ethan"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "2976 Sunset Dr"
      },
      "line2": {
        "value": "8844 Market St"
      },
      "line3": {
        "value": "9675 Lake Ave"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66853"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2253@example.com"
      },
      "phone_number": {
        "value": "3826965744"
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
content-type: application/grpc
date: Mon, 23 Mar 2026 18:28:04 GMT
x-request-id: setup_recurring_setup_recurring_req

Response contents:
{
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "508",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:28:03 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "f7f98837-27d3-43b1-8d07-b9e5c1850eb3-7572-12608461"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "525968128-538075836"
    }
  },
  "capturedAmount": "6000",
  "state": {
    "connectorCustomerId": "525968128"
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
  -H "x-request-id: recurring_charge_recurring_charge_low_amount_req" \
  -H "x-connector-request-reference-id: recurring_charge_recurring_charge_low_amount_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.RecurringPaymentService/Charge <<'JSON'
{
  "merchant_charge_id": "mchi_124696c373844ef896737356",
  "connector_recurring_payment_id": {
    "connector_mandate_id": {
      "connector_mandate_id": "525968128-538075836"
    }
  },
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_customer_id": "525968128",
  "customer": {
    "connector_customer_id": "525968128"
  }
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
x-connector-request-reference-id: recurring_charge_recurring_charge_low_amount_ref
x-merchant-id: test_merchant
x-request-id: recurring_charge_recurring_charge_low_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Mon, 23 Mar 2026 18:28:06 GMT
x-request-id: recurring_charge_recurring_charge_low_amount_req

Response contents:
{
  "connectorTransactionId": "80053224801",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "608",
    "content-type": "application/json; charset=utf-8",
    "date": "Mon, 23 Mar 2026 18:28:06 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "f7f98837-27d3-43b1-8d07-b9e5c1850eb3-7572-12608540"
  },
  "connectorFeatureData": {
    "value": "{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}}"
  },
  "networkTransactionId": "U84L11YR982844PW5JVPD9C",
  "merchantChargeId": "80053224801",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "525968128-538075836"
    }
  },
  "state": {
    "connectorCustomerId": "525968128"
  },
  "rawConnectorResponse": "***MASKED***"
  },
  "rawConnectorRequest": "***MASKED***"
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzdWx0X2NvZGUiOiJZIiwiZGVzY3JpcHRpb24iOiJUaGUgc3RyZWV0IGFkZHJlc3MgYW5kIHBvc3RhbCBjb2RlIG1hdGNoZWQuIn0="
      }
    }
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../recurring-charge.md) | [Back to Overview](../../../test_overview.md)
