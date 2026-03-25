# Connector `worldpay` / Suite `get` / Scenario `sync_payment_with_handle_response`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'connector_transaction_id': expected field to exist
```

**Pre Requisites Executed**

<details>
<summary>1. create_customer(create_customer) — FAIL</summary>

**Dependency Error**

```text
sdk call failed: sdk HTTP request failed for 'create_customer'/'create_customer': builder error
```

<details>
<summary>Show Dependency Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

_Response trace not available._

</details>

</details>
<details>
<summary>2. authorize(no3ds_auto_capture_credit_card) — FAIL</summary>

**Dependency Error**

```text
sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_credit_card': Invalid Configuration: connector_config.merchant_name (code: BAD_REQUEST)
```

<details>
<summary>Show Dependency Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Dependency Response (masked)</summary>

_Response trace not available._

</details>

</details>
<details>
<summary>Show Request (masked)</summary>

```json
{
  "connector_transaction_id": "auto_generate",
  "amount": {
    "minor_amount": 6000,
    "currency": "USD"
  }
}
```

</details>

<details>
<summary>Show Response (masked)</summary>

```json
{
  "status": "FAILURE",
  "error": {
    "issuer_details": {
      "network_details": {}
    },
    "connector_details": {
      "code": "urlContainsInvalidValue",
      "message": "Please provide a valid url value or values."
    }
  },
  "status_code": 400,
  "response_headers": {
    "x-content-type-options": "nosniff",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "wp-correlationid": "614b7181-42c7-4070-8e3b-40c5ebb126d7",
    "x-cache-hits": "0, 0",
    "x-frame-options": "DENY",
    "x-served-by": "cache-lcy-egml8630079-LCY, cache-fra-etou8220123-FRA",
    "referrer-policy": "no-referrer",
    "x-timer": "S1774269166.690702,VS0,VE169",
    "content-length": "95",
    "x-xss-protection": "0",
    "connection": "keep-alive",
    "expires": "0",
    "date": "Mon, 23 Mar 2026 12:32:45 GMT",
    "accept-ranges": "bytes",
    "pragma": "no-cache",
    "content-type": "application/json",
    "cache-control": "no-cache, no-store, max-age=0, must-revalidate"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
