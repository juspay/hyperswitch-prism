# Connector `rapyd` / Suite `PaymentService/Refund`

- Service: `PaymentService/Refund`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund \| Full Amount`](./paymentservice-refund/refund-full-amount.md) | - | - | `PASS` | `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Partial Amount`](./paymentservice-refund/refund-partial-amount.md) | - | - | `PASS` | `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Reason`](./paymentservice-refund/refund-with-reason.md) | - | - | `PASS` | `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |
