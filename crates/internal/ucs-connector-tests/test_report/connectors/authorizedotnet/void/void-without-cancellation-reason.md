# Connector `authorizedotnet` / Suite `void` / Scenario `void_without_cancellation_reason`

- Service: `PaymentService/Void`
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
  "merchant_customer_id": "mcui_9dee561d1065438088dc7a897bfe311d",
  "customer_name": "Mia Smith",
  "email": {
    "value": "morgan.6810@example.com"
  },
  "phone_number": "+11046547179",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "3238 Market Blvd"
      },
      "line2": {
        "value": "974 Lake Dr"
      },
      "line3": {
        "value": "7766 Lake Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32858"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8591@example.com"
      },
      "phone_number": {
        "value": "3431872131"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6355 Lake Blvd"
      },
      "line2": {
        "value": "7998 Lake Ave"
      },
      "line3": {
        "value": "7467 Main Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45209"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3350@example.com"
      },
      "phone_number": {
        "value": "3082072428"
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
date: Fri, 13 Mar 2026 06:46:27 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "525893660",
  "connectorCustomerId": "525893660",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "232",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:26 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4052042"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_manual_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: authorizedotnet" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_manual_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_3f6fcb0b8df44c418d5030c91e95a697",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
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
        "value": "Ethan Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "MANUAL",
  "customer": {
    "name": "Ethan Taylor",
    "email": {
      "value": "sam.4461@example.com"
    },
    "id": "cust_d67b1f75f126476e8a2e07b79eaac2dd",
    "phone_number": "+913702047980",
    "connector_customer_id": "525893660"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Miller"
      },
      "line1": {
        "value": "3238 Market Blvd"
      },
      "line2": {
        "value": "974 Lake Dr"
      },
      "line3": {
        "value": "7766 Lake Ave"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "32858"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.8591@example.com"
      },
      "phone_number": {
        "value": "3431872131"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "6355 Lake Blvd"
      },
      "line2": {
        "value": "7998 Lake Ave"
      },
      "line3": {
        "value": "7467 Main Dr"
      },
      "city": {
        "value": "Austin"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "45209"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "sam.3350@example.com"
      },
      "phone_number": {
        "value": "3082072428"
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
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Authorize a payment amount on a payment method. This reserves funds
// without capturing them, essential for verifying availability before finalizing.
rpc Authorize ( .types.PaymentServiceAuthorizeRequest ) returns ( .types.PaymentServiceAuthorizeResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: authorize_no3ds_manual_capture_credit_card_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_manual_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:28 GMT
x-request-id: authorize_no3ds_manual_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "80052927687",
  "connectorTransactionId": "80052927687",
  "status": "AUTHORIZED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "653",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:27 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "c1f4e070-bd17-40d9-a647-45299f204deb-10168-4103894"
  },
  "networkTransactionId": "Y6AXVCEILB1NRU0G1ICL4SG",
  "state": {
    "connectorCustomerId": "525893660"
  },
  "rawConnectorResponse": {
    "value": "{\"transactionResponse\":{\"responseCode\":\"1\",\"authCode\":\"72MFOR\",\"avsResultCode\":\"Y\",\"cvvResultCode\":\"P\",\"cavvResultCode\":\"2\",\"transId\":\"80052927687\",\"refTransID\":\"\",\"transHash\":\"\",\"testRequest\":\"0\",\"accountNumber\":\"XXXX1111\",\"accountType\":\"Visa\",\"messages\":[{\"code\":\"1\",\"description\":\"This transaction has been approved.\"}],\"transHashSha2\":\"F469DC8FD77B49F4164D61B8E8285FAE662FE8B6F5AA973CEFEB22462C0B8FB588063BF5F2D79A8961F030273EE252F215DA2879C724FF5222B22841838950EF\",\"SupplementalDataQualificationIndicator\":0,\"networkTransId\":\"Y6AXVCEILB1NRU0G1ICL4SG\"},\"refId\":\"\",\"messages\":{\"resultCode\":\"Ok\",\"message\":[{\"code\":\"I00001\",\"text\":\"Successful.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\"},\"body\":{\"createTransactionRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"refId\":null,\"transactionRequest\":{\"transactionType\":\"authOnlyTransaction\",\"amount\":60.0,\"currencyCode\":\"USD\",\"payment\":{\"creditCard\":{\"cardNumber\":\"4111111111111111\",\"expirationDate\":\"2030-08\",\"cardCode\":\"999\"}},\"order\":{\"invoiceNumber\":\"WlzZf62YqRFlxJHeu8I8\",\"description\":\"mti_3f6fcb0b8df44c418d5030c91e95a697\"},\"customer\":{\"id\":\"525893660\",\"email\":\"sam.4461@example.com\"},\"billTo\":{\"firstName\":\"Noah\",\"lastName\":\"Smith\",\"address\":\"6355 Lake Blvd 7998 Lake Ave 7467 Main Dr\",\"city\":\"Austin\",\"state\":\"CA\",\"zip\":\"45209\",\"country\":\"US\"}}}}}"
  },
  "connectorResponse": {
    "additionalPaymentMethodData": {
      "card": {
        "paymentChecks": "eyJhdnNfcmVzdWx0X2NvZGUiOiJZIiwiZGVzY3JpcHRpb24iOiJUaGUgc3RyZWV0IGFkZHJlc3MgYW5kIHBvc3RhbCBjb2RlIG1hdGNoZWQuIn0="
      }
    }
  },
  "connectorFeatureData": {
    "value": "{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}}"
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
  -H "x-request-id: void_void_without_cancellation_reason_req" \
  -H "x-connector-request-reference-id: void_void_without_cancellation_reason_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Void <<'JSON'
{
  "connector_transaction_id": "80052927687",
  "merchant_void_id": "mvi_718e213b21af476695eab8bea3e9a048",
  "state": {
    "connector_customer_id": "525893660"
  },
  "connector_feature_data": {
    "value": "{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}}"
  }
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Cancel an authorized payment before capture. Releases held funds back to
// customer, typically used when orders are cancelled or abandoned.
rpc Void ( .types.PaymentServiceVoidRequest ) returns ( .types.PaymentServiceVoidResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: void_void_without_cancellation_reason_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: void_void_without_cancellation_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:29 GMT
x-request-id: void_void_without_cancellation_reason_req

Response contents:
{
  "connectorTransactionId": "80052927687",
  "status": "VOIDED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "608",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:28 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4052215"
  },
  "merchantVoidId": "80052927687",
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\"},\"body\":{\"createTransactionRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"transactionRequest\":{\"transactionType\":\"voidTransaction\",\"refTransId\":\"80052927687\"}}}}"
  },
  "connectorFeatureData": {
    "value": "{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../void.md) | [Back to Overview](../../../test_overview.md)
