# Connector `stripe` / Suite `incremental_authorization`

- Service: `Unknown`
- Pass Rate: `0.0%` (`0` / `4`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Incremental Authorization \| Incremental Auth Basic`](./incremental-authorization/incremental-auth-basic.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Incremental Authorization \| Incremental Auth Fail Invalid Transaction ID`](./incremental-authorization/incremental-auth-fail-invalid-transaction-id.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Incremental Authorization \| Incremental Auth Multiple Increase`](./incremental-authorization/incremental-auth-multiple-increase.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Incremental Authorization \| Incremental Auth With Tip`](./incremental-authorization/incremental-auth-with-tip.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Incremental Authorization | Incremental Auth Basic`](./incremental-authorization/incremental-auth-basic.md) — assertion failed for field 'connector_authorization_id': ***MASKED***
- [`Incremental Authorization | Incremental Auth Fail Invalid Transaction ID`](./incremental-authorization/incremental-auth-fail-invalid-transaction-id.md) — assertion failed for field 'status': expected one of ["FAILED", "PROCESSING_ERROR"], got "AUTHORIZATION_FAILURE"
- [`Incremental Authorization | Incremental Auth Multiple Increase`](./incremental-authorization/incremental-auth-multiple-increase.md) — assertion failed for field 'status': expected one of ["SUCCESS", "AUTHORIZED", "FAILED"], got "AUTHORIZATION_FAILURE"
- [`Incremental Authorization | Incremental Auth With Tip`](./incremental-authorization/incremental-auth-with-tip.md) — assertion failed for field 'connector_authorization_id': ***MASKED***