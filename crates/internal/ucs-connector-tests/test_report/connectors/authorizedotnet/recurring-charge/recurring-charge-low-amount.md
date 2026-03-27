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
  "merchant_customer_id": "mcui_dbbb62379df3482e8c59a5111f621d7e",
  "customer_name": "Liam Brown",
  "email": {
    "value": "alex.2149@example.com"
  },
  "phone_number": "+919901303466",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Ava"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "8978 Sunset Rd"
      },
      "line2": {
        "value": "7814 Lake Dr"
      },
      "line3": {
        "value": "7413 Sunset Rd"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "67352"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.2068@example.com"
      },
      "phone_number": {
        "value": "4182595094"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3120 Market St"
      },
      "line2": {
        "value": "9188 Sunset Ln"
      },
      "line3": {
        "value": "5879 Main Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60151"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7928@example.com"
      },
      "phone_number": {
        "value": "4133549821"
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
date: Fri, 13 Mar 2026 06:46:47 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "525893671",
  "connectorCustomerId": "525893671",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "232",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:47 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4054511"
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
  -H "x-connector: authorizedotnet" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: setup_recurring_setup_recurring_req" \
  -H "x-connector-request-reference-id: setup_recurring_setup_recurring_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/SetupRecurring <<'JSON'
{
  "merchant_recurring_payment_id": "mrpi_e939354c358540f099f986059463dee1",
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
        "value": "Noah Brown"
      },
      "card_type": "credit"
    }
  },
  "customer": {
    "name": "Ethan Wilson",
    "email": {
      "value": "jordan.5651@example.com"
    },
    "id": "cust_e604d30007994b41b00342b7f61afde7",
    "phone_number": "+14360523106",
    "connector_customer_id": "525893671"
  },
  "setup_future_usage": "OFF_SESSION",
  "address": {
    "billing_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "3120 Market St"
      },
      "line2": {
        "value": "9188 Sunset Ln"
      },
      "line3": {
        "value": "5879 Main Rd"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "60151"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7928@example.com"
      },
      "phone_number": {
        "value": "4133549821"
      },
      "phone_country_code": "+91"
    }
  },
  "auth_type": "NO_THREE_DS",
  "enrolled_for_3ds": false,
  "customer_acceptance": {
    "acceptance_type": "OFFLINE"
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
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: setup_recurring_setup_recurring_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: setup_recurring_setup_recurring_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:48 GMT
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
    "content-length": "509",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:47 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4054577"
  },
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "525893671-537998897"
    }
  },
  "capturedAmount": "0",
  "state": {
    "connectorCustomerId": "525893671"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\"},\"body\":{\"createCustomerPaymentProfileRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"customerProfileId\":\"525893671\",\"paymentProfile\":{\"billTo\":{\"firstName\":\"Mia\",\"lastName\":\"Johnson\",\"address\":\"3120 Market St 9188 Sunset Ln 5879 Main Rd\",\"city\":\"Los Angeles\",\"state\":\"CA\",\"zip\":\"60151\",\"country\":\"US\"},\"payment\":{\"creditCard\":{\"cardNumber\":\"4111111111111111\",\"expirationDate\":\"2030-08\",\"cardCode\":\"999\"}}},\"validationMode\":\"testMode\"}}}"
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
  -H "x-request-id: recurring_charge_recurring_charge_low_amount_req" \
  -H "x-connector-request-reference-id: recurring_charge_recurring_charge_low_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.RecurringPaymentService/Charge <<'JSON'
{
  "merchant_charge_id": "mchi_d6b6fb4c0ea04e7c99258570525ab4e8",
  "connector_recurring_payment_id": {
    "connector_mandate_id": {
      "connector_mandate_id": "525893671-537998897"
    }
  },
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "connector_customer_id": "525893671",
  "customer": {
    "connector_customer_id": "525893671"
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
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: recurring_charge_recurring_charge_low_amount_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: recurring_charge_recurring_charge_low_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:50 GMT
x-request-id: recurring_charge_recurring_charge_low_amount_req

Response contents:
{
  "connectorTransactionId": "80052927710",
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
    "date": "Fri, 13 Mar 2026 06:46:49 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4054795"
  },
  "connectorFeatureData": {
    "value": "{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}}"
  },
  "networkTransactionId": "ESBR0OTL8CTX5G9V3DSOMJF",
  "merchantChargeId": "80052927710",
  "mandateReference": {
    "connectorMandateId": {
      "connectorMandateId": "525893671-537998897"
    }
  },
  "state": {
    "connectorCustomerId": "525893671"
  },
  "rawConnectorResponse": {
    "value": "{\"transactionResponse\":{\"responseCode\":\"1\",\"authCode\":\"9TCMFY\",\"avsResultCode\":\"Y\",\"cvvResultCode\":\"P\",\"cavvResultCode\":\"2\",\"transId\":\"80052927710\",\"refTransID\":\"\",\"transHash\":\"\",\"testRequest\":\"0\",\"accountNumber\":\"XXXX1111\",\"accountType\":\"Visa\",\"messages\":[{\"code\":\"1\",\"description\":\"This transaction has been approved.\"}],\"transHashSha2\":\"\",\"profile\":{\"customerProfileId\":\"525893671\",\"customerPaymentProfileId\":\"537998897\"},\"SupplementalDataQualificationIndicator\":0,\"networkTransId\":\"ESBR0OTL8CTX5G9V3DSOMJF\"},\"refId\":\"\",\"messages\":{\"resultCode\":\"Ok\",\"message\":[{\"code\":\"I00001\",\"text\":\"Successful.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\"},\"body\":{\"createTransactionRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"refId\":null,\"transactionRequest\":{\"transactionType\":\"authCaptureTransaction\",\"amount\":60.0,\"currencyCode\":\"USD\",\"profile\":{\"customerProfileId\":\"525893671\",\"paymentProfile\":{\"paymentProfileId\":\"537998897\"}},\"order\":{\"invoiceNumber\":\"TVZvpq6B3vd8r7K1WySL\",\"description\":\"mchi_d6b6fb4c0ea04e7c99258570525ab4e8\"},\"processingOptions\":{\"isSubsequentAuth\":true}}}}}"
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
