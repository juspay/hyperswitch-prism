# Connector `stax` / Suite `tokenize_payment_method` / Scenario `tokenize_fail_invalid_card_number`

- Service: `Unknown`
- PM / PMT: `card` / `-`
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
  "merchant_customer_id": "mcui_1b5b14cdfe1c4e89aed0709f",
  "customer_name": "Ava Smith",
  "email": {
    "value": "riley.3577@example.com"
  },
  "phone_number": "+11710968202",
  "address": {
    "shipping_address": {
      "first_name": {
        "value": "Mia"
      },
      "last_name": {
        "value": "Brown"
      },
      "line1": {
        "value": "4024 Pine Rd"
      },
      "line2": {
        "value": "3042 Lake Blvd"
      },
      "line3": {
        "value": "8206 Main St"
      },
      "city": {
        "value": "Los Angeles"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "41108"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "riley.4388@sandbox.example.com"
      },
      "phone_number": {
        "value": "6797994885"
      },
      "phone_country_code": "+91"
    },
    "billing_address": {
      "first_name": {
        "value": "Liam"
      },
      "last_name": {
        "value": "Taylor"
      },
      "line1": {
        "value": "2068 Oak Dr"
      },
      "line2": {
        "value": "1549 Main Ave"
      },
      "line3": {
        "value": "8342 Oak Ln"
      },
      "city": {
        "value": "New York"
      },
      "state": {
        "value": "CA"
      },
      "zip_code": {
        "value": "66185"
      },
      "country_alpha2_code": "US",
      "email": {
        "value": "morgan.4315@example.com"
      },
      "phone_number": {
        "value": "2978093378"
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
date: Tue, 24 Mar 2026 07:33:35 GMT
x-request-id: create_customer_create_customer_req

Response contents:
{
  "merchantCustomerId": "f70ea5ea-aa02-460d-88c7-1f5ab6e1f559",
  "connectorCustomerId": "f70ea5ea-aa02-460d-88c7-1f5ab6e1f559",
  "statusCode": 200,
  "responseHeaders": {
    "access-control-allow-origin": "*",
    "access-control-expose-headers": "*",
    "cache-control": "no-cache, private",
    "cf-cache-status": "DYNAMIC",
    "cf-ray": "9e13fd092b574734-BOM",
    "connection": "keep-alive",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:33:34 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=.p9LkDH4n2MRHFwHQwn1gtcsP.zH.QM39v_ayiJpwf4-1774337614.271358-1.0.1.1-kEBZhqIEI2wWeDIMR6BQKamEF.ABlMcp6NAh0r8pvNxNJW4ijuOJoxWeVnOehkF6z_97kZkhPXvPsGl8POycgV.KO.LytDP3.5XTatcCy8sIZdNDNelvv3VTO5Wr1t1P; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:03:34 GMT",
    "transfer-encoding": "chunked",
    "x-powered-by": "PHP/8.3.11"
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
  -H "x-request-id: tokenize_payment_method_tokenize_fail_invalid_card_number_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_invalid_card_number_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_868177",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "1234567890123456"
      },
      "card_exp_month": {
        "value": "12"
      },
      "card_exp_year": {
        "value": "2030"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Invalid Card"
      }
    }
  },
  "customer": {
    "id": "cust_60095b9e5bdd486689f058d4",
    "name": "Ava Wilson",
    "email": {
      "value": "jordan.7143@testmail.io"
    },
    "connector_customer_id": "f70ea5ea-aa02-460d-88c7-1f5ab6e1f559"
  },
  "test_mode": true
}
JSON
```

</details>

<details>
<summary>Show Response (masked)</summary>

```text
Resolved method descriptor:
// Tokenize payment method for secure storage. Replaces raw card details
// with secure token for one-click payments and recurring billing.
rpc Tokenize ( .types.PaymentMethodServiceTokenizeRequest ) returns ( .types.PaymentMethodServiceTokenizeResponse );

Request metadata to send:
x-connector-config: ***MASKED***
x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_invalid_card_number_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_fail_invalid_card_number_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 07:33:38 GMT
x-request-id: tokenize_payment_method_tokenize_fail_invalid_card_number_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "422",
      "message": "Invalid Card Number",
      "reason": "{\"card_number\":[\"Invalid Card Number\"]}"
    }
  },
  "statusCode": 422,
  "responseHeaders": {
    "cf-ray": "9e13fd2499454734-BOM",
    "connection": "keep-alive",
    "content-length": "39",
    "content-type": "application/json",
    "date": "Tue, 24 Mar 2026 07:33:38 GMT",
    "server": "cloudflare",
    "set-cookie": "__cf_bm=6zf_G9sT3pSrpHz_mwPhYcpO8ZcLHK09R3TUVroWr3E-1774337618.6536696-1.0.1.1-x1ZiBYnjC4RVsXlFeT9ekMMUD0Evbsckk.gV1hxCRzrB0mQ8IKFenrQKjTg04CruaO_d6GCweSJx7q6VI.iqwJ6mUbh4NgZtOKG_cx8GVjPQQ3Gbm8RyxJDd60yHixH2; HttpOnly; Secure; Path=/; Domain=fattlabs.com; Expires=Tue, 24 Mar 2026 08:03:38 GMT",
    "vary": "accept-encoding"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)
