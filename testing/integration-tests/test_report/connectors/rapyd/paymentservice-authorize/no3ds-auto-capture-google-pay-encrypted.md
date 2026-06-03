# Connector `rapyd` / Suite `PaymentService/Authorize` / Scenario `Google Pay (Encrypted Token) | No 3DS | Automatic Capture`

- Service: `PaymentService/Authorize`
- Scenario Key: `no3ds_auto_capture_google_pay_encrypted`
- PM / PMT: `google_pay` / `CARD`
- Result: `SKIP`

**Error**

```text
credentials for connector 'rapyd' do not include metadata.google_pay; add a `metadata.google_pay` block under `rapyd` in '/Users/amitsingh.tanwar/Documents/connector-service/connector-service/creds.json'. Refer to `browser-automation-engine/src/gpay-token-gen.ts` for the expected shape and use any existing connector entry in `creds.json` that already has `metadata.google_pay` as a template
```

**Pre Requisites Executed**

- None
<details>
<summary>Show Request (masked)</summary>

_Request trace not available._

</details>

<details>
<summary>Show Response (masked)</summary>

_Response trace not available._

</details>


[Back to Connector Suite](../paymentservice-authorize.md) | [Back to Overview](../../../test_overview.md)
