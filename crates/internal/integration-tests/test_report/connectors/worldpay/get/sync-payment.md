# Connector `worldpay` / Suite `get` / Scenario `sync_payment`

- Service: `PaymentService/Get`
- PM / PMT: `-` / `-`
- Result: `FAIL`

**Error**

```text
assertion failed for field 'error': expected field to be absent or null, got {"issuer_details":{"network_details":{}},"connector_details":{"code":"urlContainsInvalidValue","message":"Please provide a valid url value or values."}}
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
    "x-frame-options": "DENY",
    "expires": "0",
    "accept-ranges": "bytes",
    "content-length": "95",
    "strict-transport-security": "max-age=31536000 ; includeSubDomains",
    "x-served-by": "cache-lcy-egml8630079-LCY, cache-fra-etou8220198-FRA",
    "wp-correlationid": "faee6a90-69eb-4e20-93e3-4a5771f4ab9e",
    "connection": "keep-alive",
    "content-type": "application/json",
    "x-content-type-options": "nosniff",
    "pragma": "no-cache",
    "referrer-policy": "no-referrer",
    "date": "Mon, 23 Mar 2026 12:32:45 GMT",
    "x-cache-hits": "0, 0",
    "x-timer": "S1774269165.111614,VS0,VE85",
    "cache-control": "no-cache, no-store, max-age=0, must-revalidate",
    "x-xss-protection": "0"
  },
  "raw_connector_response": "***MASKED***"
}
```

</details>


[Back to Connector Suite](../get.md) | [Back to Overview](../../../test_overview.md)
