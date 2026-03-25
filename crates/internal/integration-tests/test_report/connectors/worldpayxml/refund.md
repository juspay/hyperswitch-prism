# Connector `worldpayxml` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_full_amount`](./refund/refund-full-amount.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`refund_partial_amount`](./refund/refund-partial-amount.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`refund_with_reason`](./refund/refund-with-reason.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
