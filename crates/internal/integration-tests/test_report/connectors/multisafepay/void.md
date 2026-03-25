# Connector `multisafepay` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`void_authorized_payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`void_with_amount`](./void/void-with-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`void_authorized_payment`](./void/void-authorized-payment.md) — Resolved method descriptor:
- [`void_with_amount`](./void/void-with-amount.md) — Resolved method descriptor:
- [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) — Resolved method descriptor:
