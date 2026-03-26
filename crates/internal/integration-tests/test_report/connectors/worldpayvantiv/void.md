# Connector `worldpayvantiv` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Amount`](./void/void-with-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Without Cancellation Reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Void | Authorized Payment`](./void/void-authorized-payment.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "VOID_INITIATED"
- [`Void | Amount`](./void/void-with-amount.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "VOID_INITIATED"
- [`Void | Without Cancellation Reason`](./void/void-without-cancellation-reason.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "VOID_INITIATED"