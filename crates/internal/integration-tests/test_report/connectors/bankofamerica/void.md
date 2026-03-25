# Connector `bankofamerica` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `66.7%` (`2` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`void_authorized_payment`](./void/void-authorized-payment.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_with_amount`](./void/void-with-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) — Resolved method descriptor:
