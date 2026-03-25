# Connector `hipay` / Suite `tokenize_payment_method` / Scenario `tokenize_fail_expired_card`

- Service: `Unknown`
- PM / PMT: `card` / `-`
- Result: `PASS`

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

```bash
grpcurl -plaintext \
  -H "x-merchant-id: test_merchant" \
  -H "x-tenant-id: default" \
  -H "x-request-id: tokenize_payment_method_tokenize_fail_expired_card_req" \
  -H "x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_expired_card_ref" \
  -H "x-connector-config: ***MASKED***" \
  -d @ localhost:50051 types.PaymentMethodService/Tokenize <<'JSON'
{
  "merchant_payment_method_id": "gen_722071",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  },
  "payment_method": {
    "card": {
      "card_number": ***MASKED***
        "value": "4242424242424242"
      },
      "card_exp_month": {
        "value": "01"
      },
      "card_exp_year": {
        "value": "2020"
      },
      "card_cvc": ***MASKED***
        "value": "123"
      },
      "card_holder_name": {
        "value": "Expired Card"
      }
    }
  },
  "customer": {
    "id": "cust_97b5fffa0138426cae437de0",
    "name": "Noah Johnson",
    "email": {
      "value": "sam.3960@example.com"
    }
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
x-connector-request-reference-id: tokenize_payment_method_tokenize_fail_expired_card_ref
x-merchant-id: test_merchant
x-request-id: tokenize_payment_method_tokenize_fail_expired_card_req
x-tenant-id: default

Response headers received:
content-type: application/grpc
date: Tue, 24 Mar 2026 05:49:06 GMT
x-request-id: tokenize_payment_method_tokenize_fail_expired_card_req

Response contents:
{
  "error": {
    "connectorDetails": {
      "code": "400",
      "message": "\u003c!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\"\u003e\n\u003chtml\u003e\u003chead\u003e\n\u003ctitle\u003e400 Bad Request\u003c/title\u003e\n\u003c/head\u003e\u003cbody\u003e\n\u003ch1\u003eBad Request\u003c/h1\u003e\n\u003cp\u003eYour browser sent a request...",
      "reason": "\u003c!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\"\u003e\n\u003chtml\u003e\u003chead\u003e\n\u003ctitle\u003e400 Bad Request\u003c/title\u003e\n\u003c/head\u003e\u003cbody\u003e\n\u003ch1\u003eBad Request\u003c/h1\u003e\n\u003cp\u003eYour browser sent a request..."
    }
  },
  "statusCode": 400,
  "responseHeaders": {
    "connection": "keep-alive",
    "content-length": "266",
    "content-type": "text/html; charset=iso-8859-1",
    "date": "Tue, 24 Mar 2026 05:49:06 GMT",
    "keep-alive": "timeout=30",
    "server": "nginx"
  }
}

Response trailers received:
(empty)
Sent 1 request and received 1 response
```

</details>


[Back to Connector Suite](../tokenize-payment-method.md) | [Back to Overview](../../../test_overview.md)