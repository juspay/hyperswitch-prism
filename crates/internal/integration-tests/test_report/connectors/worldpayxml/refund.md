# Connector `worldpayxml` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund \| Full Amount`](./refund/refund-full-amount.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Refund \| Partial Amount`](./refund/refund-partial-amount.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Refund \| Reason`](./refund/refund-with-reason.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |