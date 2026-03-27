# Connector `authorizedotnet` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
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
  "merchant_customer_id": "mcui_0d2dd0463ae547f5bd3f286e5ea0c6d9",
  "customer_name": "Mia Johnson",
  "email": {
    "value": "morgan.7665@sandbox.example.com"
  },
  "phone_number": "+18922166074",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8534 Main Ln"
      },
      "line2": {
        "value": "2538 Oak St"
      },
      "line3": {
        "value": "5672 Oak Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83704"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7949@sandbox.example.com"
      },
      "phone_number": {
        "value": "3076761916"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "1664 Main Ave"
      },
      "line2": {
        "value": "3993 Pine Blvd"
      },
      "line3": {
        "value": "5442 Sunset Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "15006"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4858@testmail.io"
      },
      "phone_number": {
        "value": "3207871226"
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
date: Fri, 13 Mar 2026 06:46:38 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "525893666",
  "connectorCustomerId": "525893666",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "232",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:36 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "c1f4e070-bd17-40d9-a647-45299f204deb-10168-4105148"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: authorizedotnet" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: authorize_no3ds_auto_capture_credit_card_req" \
  -H "x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Authorize <<'JSON'
{
  "merchant_transaction_id": "mti_914b5e82fbf043f6a38c37fe92fe98fd",
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
        "value": "Noah Miller"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Ethan Miller",
    "email": {
      "value": "alex.8618@sandbox.example.com"
    },
    "id": "cust_c3601939aa1c43ae9001bb18e2d949db",
    "phone_number": "+441793656072",
    "connector_customer_id": "525893666"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Smith"
      },
      "line1": {
        "value": "8534 Main Ln"
      },
      "line2": {
        "value": "2538 Oak St"
      },
      "line3": {
        "value": "5672 Oak Rd"
      },
      "city": {
        "value": "Chicago"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "83704"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.7949@sandbox.example.com"
      },
      "phone_number": {
        "value": "3076761916"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Emma"
      },
      "last_name": {
        "value": "Johnson"
      },
      "line1": {
        "value": "1664 Main Ave"
      },
      "line2": {
        "value": "3993 Pine Blvd"
      },
      "line3": {
        "value": "5442 Sunset Ln"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "15006"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "alex.4858@testmail.io"
      },
      "phone_number": {
        "value": "3207871226"
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
  "description": "No3DS auto capture card payment (credit)",
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
x-connector-request-reference-id: authorize_no3ds_auto_capture_credit_card_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: authorize_no3ds_auto_capture_credit_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:38 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "80052927698",
  "connectorTransactionId": "80052927698",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "653",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:38 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4053425"
  },
  "networkTransactionId": "GENOEUQW838HUPO30BY2ID7",
  "state": {
    "connectorCustomerId": "525893666"
  },
  "rawConnectorResponse": {
    "value": "{\"transactionResponse\":{\"responseCode\":\"1\",\"authCode\":\"961X5E\",\"avsResultCode\":\"Y\",\"cvvResultCode\":\"P\",\"cavvResultCode\":\"2\",\"transId\":\"80052927698\",\"refTransID\":\"\",\"transHash\":\"\",\"testRequest\":\"0\",\"accountNumber\":\"XXXX1111\",\"accountType\":\"Visa\",\"messages\":[{\"code\":\"1\",\"description\":\"This transaction has been approved.\"}],\"transHashSha2\":\"0A09AB5DA8AACDD18CEF511B4EF181A116B730AA675DA5421FEF8757FF2AA7710F71A670DF0A63CAB0A0A4C8A9798F3136F149160EBD078FD135E70696566675\",\"SupplementalDataQualificationIndicator\":0,\"networkTransId\":\"GENOEUQW838HUPO30BY2ID7\"},\"refId\":\"\",\"messages\":{\"resultCode\":\"Ok\",\"message\":[{\"code\":\"I00001\",\"text\":\"Successful.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\"},\"body\":{\"createTransactionRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"refId\":null,\"transactionRequest\":{\"transactionType\":\"authCaptureTransaction\",\"amount\":60.0,\"currencyCode\":\"USD\",\"payment\":{\"creditCard\":{\"cardNumber\":\"4111111111111111\",\"expirationDate\":\"2030-08\",\"cardCode\":\"999\"}},\"order\":{\"invoiceNumber\":\"1nCuaznaxXsmsE8dJ8Pw\",\"description\":\"mti_914b5e82fbf043f6a38c37fe92fe98fd\"},\"customer\":{\"id\":\"525893666\",\"email\":\"alex.8618@sandbox.example.com\"},\"billTo\":{\"firstName\":\"Emma\",\"lastName\":\"Johnson\",\"address\":\"1664 Main Ave 3993 Pine Blvd 5442 Sunset Ln\",\"city\":\"Los Angeles\",\"state\":\"CA\",\"zip\":\"15006\",\"country\":\"US\"}}}}}"
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
  -H "x-request-id: get_sync_payment_with_handle_response_req" \
  -H "x-connector-request-reference-id: get_sync_payment_with_handle_response_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Get <<'JSON'
{
  "connector_transaction_id": "80052927698",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "525893666"
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
// Retrieve current payment status from the payment processor. Enables synchronization
// between your system and payment processors for accurate state tracking.
rpc Get ( .types.PaymentServiceGetRequest ) returns ( .types.PaymentServiceGetResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: get_sync_payment_with_handle_response_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: get_sync_payment_with_handle_response_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:39 GMT
x-request-id: get_sync_payment_with_handle_response_req

Response contents:
{
  "connectorTransactionId": "80052927698",
  "status": "CHARGED",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "1187",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:39 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "c1f4e070-bd17-40d9-a647-45299f204deb-10168-4105341"
  },
  "rawConnectorResponse": {
    "value": "{\"transaction\":{\"transId\":\"80052927698\",\"submitTimeUTC\":\"2026-03-13T06:46:38.603Z\",\"submitTimeLocal\":\"2026-03-12T23:46:38.603\",\"transactionType\":\"authCaptureTransaction\",\"transactionStatus\":\"capturedPendingSettlement\",\"responseCode\":1,\"responseReasonCode\":1,\"responseReasonDescription\":\"Approval\",\"authCode\":\"961X5E\",\"AVSResponse\":\"Y\",\"cardCodeResponse\":\"P\",\"order\":{\"invoiceNumber\":\"1nCuaznaxXsmsE8dJ8Pw\",\"description\":\"mti_914b5e82fbf043f6a38c37fe92fe98fd\",\"discountAmount\":0.0,\"taxIsAfterDiscount\":false},\"authAmount\":60.00,\"settleAmount\":60.00,\"taxExempt\":false,\"payment\":{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\",\"cardType\":\"Visa\"}},\"customer\":{\"id\":\"525893666\",\"email\":\"alex.8618@sandbox.example.com\"},\"billTo\":{\"firstName\":\"Emma\",\"lastName\":\"Johnson\",\"address\":\"1664 Main Ave 3993 Pine Blvd 5442 Sunset Ln\",\"city\":\"Los Angeles\",\"state\":\"CA\",\"zip\":\"15006\",\"country\":\"US\"},\"recurringBilling\":false,\"customerIP\":\"103.197.75.3\",\"product\":\"Card Not Present\",\"marketType\":\"eCommerce\",\"networkTransId\":\"GENOEUQW838HUPO30BY2ID7\",\"authorizationIndicator\":\"final\",\"tapToPhone\":false},\"messages\":{\"resultCode\":\"Ok\",\"message\":[{\"code\":\"I00001\",\"text\":\"Successful.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\"},\"body\":{\"getTransactionDetailsRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"transId\":\"80052927698\"}}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
