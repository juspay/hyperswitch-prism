# Connector `nexixpay` / Suite `authorize` / Scenario `no3ds_auto_capture_google_pay_encrypted`

- Service: `PaymentService/Authorize`
- PM / PMT: `google_pay` / `CARD`
- Result: `FAIL`

**Error**

```text
grpcurl execution failed: [google_pay_token_gen] nexixpay/authorize/no3ds_auto_capture_google_pay_encrypted: npm exited with exit status: 1
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
<summary>Show Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../authorize.md) | [Back to Overview](../../../test_overview.md)
