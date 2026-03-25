# Connector `nexixpay` / Suite `authorize` / Scenario `no3ds_auto_capture_bancontact`

- Service: `PaymentService/Authorize`
- PM / PMT: `bancontact_card` / `-`
- Result: `FAIL`

**Error**

```text
sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_bancontact': Payment method BankRedirect(BancontactCard { card_number: Some(CardNumber(411111**********)), card_exp_month: Some(*** alloc::string::String ***), card_exp_year: Some(*** alloc::string::String ***), card_holder_name: Some(*** alloc::string::String ***) }) is not supported by Nexixpay (code: BAD_REQUEST)
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
