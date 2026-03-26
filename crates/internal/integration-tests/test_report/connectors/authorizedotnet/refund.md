# Connector `authorizedotnet` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund \| Full Amount`](./refund/refund-full-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Partial Amount`](./refund/refund-partial-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Reason`](./refund/refund-with-reason.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |