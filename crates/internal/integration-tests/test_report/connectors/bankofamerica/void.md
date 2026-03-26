# Connector `bankofamerica` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `66.7%` (`2` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./void/void-authorized-payment.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Amount`](./void/void-with-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Without Cancellation Reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Void | Without Cancellation Reason`](./void/void-without-cancellation-reason.md) — Resolved method descriptor: