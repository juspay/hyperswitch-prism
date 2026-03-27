# Connector `authorizedotnet` / Suite `setup_recurring` / Scenario `setup_recurring_with_webhook`

- Service: `PaymentService/SetupRecurring`
- PM / PMT: `card` / `credit`
- Result: `PASS`

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: authorizedotnet" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: create_customer_create_customer_req" \
  -H "x-connector-request-reference-id: create_customer_create_customer_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.CustomerService/Create <<'JSON'
{
  "merchant_customer_id": "mcui_db7fc976a8e441fbb4d15c2649a5fe01",
  "customer_name": "Ava Miller",
  "email": {
    "value": "sam.4640@testmail.io"
  },
  "phone_number": "+448289811420",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "6910 Oak Rd"
      },
      "line2": {
        "value": "7833 Lake Ave"
      },
      "line3": {
        "value": "5269 Sunset Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "35274"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.1332@testmail.io"
      },
      "phone_number": {
        "value": "9702886636"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5248 Lake Ave"
      },
      "line2": {
        "value": "6278 Market Dr"
      },
      "line3": {
        "value": "6361 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27477"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5390@testmail.io"
      },
      "phone_number": {
        "value": "4671476965"
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
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: create_customer_create_customer_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: create_customer_create_customer_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:45 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "525893669",
  "connectorCustomerId": "525893669",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "232",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:44 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4054179"
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
  -H "x-connector: authorizedotnet" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_with_webhook_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_9dcf5943865142c59cd2c01fbdd88a82",
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
    "name": "Ethan Johnson",
    "email": {
      "value": "riley.2337@sandbox.example.com"
    },
    "id": "cust_8f8d414d33f3427abd9f3db387a82c85",
    "phone_number": "+445956957286",
    "connector_customer_id": "525893669"
  },
  "webhook_url": "https://example.com/payment/webhook",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "5248 Lake Ave"
      },
      "line2": {
        "value": "6278 Market Dr"
      },
      "line3": {
        "value": "6361 Market Ave"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27477"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.5390@testmail.io"
      },
      "phone_number": {
        "value": "4671476965"
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
  "return_url": "https://example.com/payment/return"
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
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: setup_recurring_setup_recurring_with_webhook_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_with_webhook_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:47 GMT
x-request-id: setup_recurring_setup_recurring_with_webhook_req

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
    "content-length": "552",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:47 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "c1f4e070-bd17-40d9-a647-45299f204deb-10168-4106245"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "525893669-537998895"
    }
  },
  "capturedAmount": "0",
  "state": {
    "connectorCustomerId": "525893669"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\"},\"body\":{\"createCustomerPaymentProfileRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"customerProfileId\":\"525893669\",\"paymentProfile\":{\"billTo\":{\"firstName\":\"Liam\",\"lastName\":\"Johnson\",\"address\":\"5248 Lake Ave 6278 Market Dr 6361 Market Ave\",\"city\":\"Seattle\",\"state\":\"CA\",\"zip\":\"27477\",\"country\":\"US\"},\"payment\":{\"creditCard\":{\"cardNumber\":\"4111111111111111\",\"expirationDate\":\"2030-08\",\"cardCode\":\"999\"}}},\"validationMode\":\"testMode\"}}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../setup-recurring.md) | [Back to Overview](../../../test_overview.md)
