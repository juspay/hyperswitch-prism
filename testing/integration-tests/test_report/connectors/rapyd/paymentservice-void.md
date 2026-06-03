# Connector `rapyd` / Suite `PaymentService/Void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./paymentservice-void/void-authorized-payment.md) | - | - | `FAIL` | `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Amount`](./paymentservice-void/void-with-amount.md) | - | - | `FAIL` | `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Without Cancellation Reason`](./paymentservice-void/void-without-cancellation-reason.md) | - | - | `FAIL` | `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Void | Authorized Payment`](./paymentservice-void/void-authorized-payment.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
- [`Void | Amount`](./paymentservice-void/void-with-amount.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
- [`Void | Without Cancellation Reason`](./paymentservice-void/void-without-cancellation-reason.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
