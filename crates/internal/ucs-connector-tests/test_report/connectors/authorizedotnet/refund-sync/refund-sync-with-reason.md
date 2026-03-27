# Connector `authorizedotnet` / Suite `refund_sync` / Scenario `refund_sync_with_reason`

- Service: `RefundService/Get`
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
  "merchant_customer_id": "mcui_2018b368c3e6470eb90248abbfa6e9dc",
  "customer_name": "Ava Johnson",
  "email": {
    "value": "riley.2787@example.com"
  },
  "phone_number": "+444750800864",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "226 Market Blvd"
      },
      "line2": {
        "value": "860 Sunset Rd"
      },
      "line3": {
        "value": "6185 Market St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27728"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9512@testmail.io"
      },
      "phone_number": {
        "value": "6089495176"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5158 Main Ave"
      },
      "line2": {
        "value": "1548 Lake St"
      },
      "line3": {
        "value": "2008 Pine Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91276"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6790@testmail.io"
      },
      "phone_number": {
        "value": "1536091896"
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
date: Fri, 13 Mar 2026 06:46:42 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "525893668",
  "connectorCustomerId": "525893668",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "232",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:42 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4053883"
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
  "merchant_transaction_id": "mti_7f8bd983ac284a23ac6197101483344f",
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
        "value": "Ethan Taylor"
      },
      "card_type": "credit"
    }
  },
  "capture_method": "AUTOMATIC",
  "customer": {
    "name": "Noah Taylor",
    "email": {
      "value": "morgan.5586@example.com"
    },
    "id": "cust_b80ed9ef1ee14c5cad3dbcf19ba108d5",
    "phone_number": "+16544256513",
    "connector_customer_id": "525893668"
  },
  "locale": "en-US",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Wilson"
      },
      "line1": {
        "value": "226 Market Blvd"
      },
      "line2": {
        "value": "860 Sunset Rd"
      },
      "line3": {
        "value": "6185 Market St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "27728"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "casey.9512@testmail.io"
      },
      "phone_number": {
        "value": "6089495176"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Noah"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "5158 Main Ave"
      },
      "line2": {
        "value": "1548 Lake St"
      },
      "line3": {
        "value": "2008 Pine Dr"
      },
      "city": {
        "value": "Seattle"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "91276"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.6790@testmail.io"
      },
      "phone_number": {
        "value": "1536091896"
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
date: Fri, 13 Mar 2026 06:46:43 GMT
x-request-id: authorize_no3ds_auto_capture_credit_card_req

Response contents:
{
  "merchantTransactionId": "80052927704",
  "connectorTransactionId": "80052927704",
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
    "date": "Fri, 13 Mar 2026 06:46:43 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4053962"
  },
  "networkTransactionId": "FZGR5N9M7QTMMKQ9PEATXPT",
  "state": {
    "connectorCustomerId": "525893668"
  },
  "rawConnectorResponse": {
    "value": "{\"transactionResponse\":{\"responseCode\":\"1\",\"authCode\":\"CCX72F\",\"avsResultCode\":\"Y\",\"cvvResultCode\":\"P\",\"cavvResultCode\":\"2\",\"transId\":\"80052927704\",\"refTransID\":\"\",\"transHash\":\"\",\"testRequest\":\"0\",\"accountNumber\":\"XXXX1111\",\"accountType\":\"Visa\",\"messages\":[{\"code\":\"1\",\"description\":\"This transaction has been approved.\"}],\"transHashSha2\":\"C9F31C6B12216CE6C36F19E8DEA0723196B0DB1BB6EAA91059B22506EB9099D9F33D438E47BA3392597F6A0C17AD3997F26E40CF78EF3FF8AFB7E49A46CD03B0\",\"SupplementalDataQualificationIndicator\":0,\"networkTransId\":\"FZGR5N9M7QTMMKQ9PEATXPT\"},\"refId\":\"\",\"messages\":{\"resultCode\":\"Ok\",\"message\":[{\"code\":\"I00001\",\"text\":\"Successful.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"Content-Type\":\"application/json\",\"via\":\"HyperSwitch\"},\"body\":{\"createTransactionRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"refId\":null,\"transactionRequest\":{\"transactionType\":\"authCaptureTransaction\",\"amount\":60.0,\"currencyCode\":\"USD\",\"payment\":{\"creditCard\":{\"cardNumber\":\"4111111111111111\",\"expirationDate\":\"2030-08\",\"cardCode\":\"999\"}},\"order\":{\"invoiceNumber\":\"innk87EU8mta1otVEu5c\",\"description\":\"mti_7f8bd983ac284a23ac6197101483344f\"},\"customer\":{\"id\":\"525893668\",\"email\":\"morgan.5586@example.com\"},\"billTo\":{\"firstName\":\"Noah\",\"lastName\":\"Brown\",\"address\":\"5158 Main Ave 1548 Lake St 2008 Pine Dr\",\"city\":\"Seattle\",\"state\":\"CA\",\"zip\":\"91276\",\"country\":\"US\"}}}}}"
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
<summary>3. refund(refund_full_amount) — PASS</summary>

<details>
<summary>Show Dependency Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-connector: authorizedotnet" \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: refund_refund_full_amount_req" \
  -H "x-connector-request-reference-id: refund_refund_full_amount_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.PaymentService/Refund <<'JSON'
{
  "merchant_refund_id": "mri_c33ee65ad37947c5a8c615a5b3a4843c",
  "connector_transaction_id": "80052927704",
  "payment_amount": 6000,
  "refund_amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "state": {
    "connector_customer_id": "525893668"
  },
  "connector_feature_data": {
    "value": "{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}}"
  }
}
JSON
```

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

```text
Resolved method descriptor:
// Initiate a refund to customer's payment method. Returns funds for
// returns, cancellations, or service adjustments after original payment.
rpc Refund ( .types.PaymentServiceRefundRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: refund_refund_full_amount_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: refund_refund_full_amount_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:44 GMT
x-request-id: refund_refund_full_amount_req

Response contents:
{
  "status": 21,
  "error": {
    "connectorDetails": {
      "code": "54",
      "message": "The referenced transaction does not meet the criteria for issuing a credit.",
      "reason": "The referenced transaction does not meet the criteria for issuing a credit."
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "659",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:43 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "53649f1c-cce8-4d4d-9a6c-f2139cb719a0-17392-4054065"
  },
  "connectorTransactionId": "0",
  "rawConnectorResponse": {
    "value": "{\"transactionResponse\":{\"responseCode\":\"3\",\"authCode\":\"\",\"avsResultCode\":\"P\",\"cvvResultCode\":\"\",\"cavvResultCode\":\"\",\"transId\":\"0\",\"refTransID\":\"80052927704\",\"transHash\":\"\",\"testRequest\":\"0\",\"accountNumber\":\"XXXX1111\",\"accountType\":\"Visa\",\"errors\":[{\"errorCode\":\"54\",\"errorText\":\"The referenced transaction does not meet the criteria for issuing a credit.\"}],\"transHashSha2\":\"DD3364110A093C5B08CE258A80B1450D9C5F96D5660F945A2BD49A1313FF4686A48816A23495ABCF6ADEB86E932EB19953A4E582EB3CE124A9C915015BBABCD7\",\"SupplementalDataQualificationIndicator\":0},\"messages\":{\"resultCode\":\"Error\",\"message\":[{\"code\":\"E00027\",\"text\":\"The transaction was unsuccessful.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\"},\"body\":{\"createTransactionRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"transactionRequest\":{\"transactionType\":\"refundTransaction\",\"amount\":60.0,\"currencyCode\":\"USD\",\"payment\":{\"creditCard\":{\"cardNumber\":\"XXXX1111\",\"expirationDate\":\"XXXX\"}},\"refTransId\":\"80052927704\"}}}}"
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
  -H "x-request-id: refund_sync_refund_sync_with_reason_req" \
  -H "x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref" \
  -H "x-auth: ***MASKED***" \
  -H "x-api-key: ***MASKED***" \
  -H "x-key1: ***MASKED***" \
  -d @ localhost:8000 types.RefundService/Get <<'JSON'
{
  "connector_transaction_id": "0",
  "refund_id": "authnet_refund_reference",
  "state": {
    "connector_customer_id": "525893668"
  },
  "refund_reason": "customer_requested",
  "merchant_refund_id": "mri_c33ee65ad37947c5a8c615a5b3a4843c",
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
// Retrieve refund status from the payment processor. Tracks refund progress
// through processor settlement for accurate customer communication.
rpc Get ( .types.RefundServiceGetRequest ) returns ( .types.RefundResponse );

Request metadata to send:
x-api-key: ***MASKED***
x-auth: ***MASKED***
x-connector: authorizedotnet
x-connector-request-reference-id: refund_sync_refund_sync_with_reason_ref
x-key1: ***MASKED***
x-merchant-id: test_merchant
x-request-id: refund_sync_refund_sync_with_reason_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Fri, 13 Mar 2026 06:46:44 GMT
x-request-id: refund_sync_refund_sync_with_reason_req

Response contents:
{
  "status": 21,
  "error": {
    "connectorDetails": {
      "code": "E00003",
      "message": "The 'AnetApi/xml/v1/schema/AnetApiSchema.xsd:transId' element is invalid - The value \u0026#39;authnet_refund_reference\u0026#39; is invalid according to its datatype 'AnetApi/xml/v1/schema/AnetApiSchema.xsd:numericString' - The Pattern constraint failed.",
      "reason": "The 'AnetApi/xml/v1/schema/AnetApiSchema.xsd:transId' element is invalid - The value \u0026#39;authnet_refund_reference\u0026#39; is invalid according to its datatype 'AnetApi/xml/v1/schema/AnetApiSchema.xsd:numericString' - The Pattern constraint failed."
    }
  },
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-credentials": "true",
    "access-control-allow-headers": "x-requested-with,cache-control,content-type,origin,method,SOAPAction",
    "access-control-allow-methods": "PUT,OPTIONS,POST,GET",
    "access-control-allow-origin": "*",
    "cache-control": "no-cache, no-store, max-age=0",
    "content-length": "323",
    "content-type": "application/json; charset=utf-8",
    "date": "Fri, 13 Mar 2026 06:46:44 GMT",
    "expires": "-1",
    "pragma": "no-cache",
    "strict-transport-security": "max-age=31536000; includeSubDomains",
    "x-cnection": "close",
    "x-download-options": "noopen",
    "x-opnet-transaction-trace": "c1f4e070-bd17-40d9-a647-45299f204deb-10168-4105959"
  },
  "rawConnectorResponse": {
    "value": "{\"messages\":{\"resultCode\":\"Error\",\"message\":[{\"code\":\"E00003\",\"text\":\"The 'AnetApi/xml/v1/schema/AnetApiSchema.xsd:transId' element is invalid - The value \u0026#39;authnet_refund_reference\u0026#39; is invalid according to its datatype 'AnetApi/xml/v1/schema/AnetApiSchema.xsd:numericString' - The Pattern constraint failed.\"}]}}"
  },
  "rawConnectorRequest": {
    "value": "{\"url\":\"https://apitest.authorize.net/xml/v1/request.api\",\"method\":\"POST\",\"headers\":{\"via\":\"HyperSwitch\",\"Content-Type\":\"application/json\"},\"body\":{\"getTransactionDetailsRequest\":{\"merchantAuthentication\":{\"name\":\"9ARWr5wz49D3\",\"transactionKey\":\"3d9SHV2699mgmHre\"},\"transId\":\"authnet_refund_reference\"}}}"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../refund-sync.md) | [Back to Overview](../../../test_overview.md)
