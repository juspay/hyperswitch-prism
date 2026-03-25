# Connector `rapyd` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`void_authorized_payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_with_amount`](./void/void-with-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`void_authorized_payment`](./void/void-authorized-payment.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
- [`void_with_amount`](./void/void-with-amount.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
- [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) — assertion failed for field 'status': expected one of ["VOIDED", "PENDING"], got "AUTHORIZED"
