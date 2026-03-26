# Connector `helcim` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Void \| Amount`](./void/void-with-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Void \| Without Cancellation Reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Void | Authorized Payment`](./void/void-authorized-payment.md) — Resolved method descriptor:
- [`Void | Amount`](./void/void-with-amount.md) — Resolved method descriptor:
- [`Void | Without Cancellation Reason`](./void/void-without-cancellation-reason.md) — Resolved method descriptor: