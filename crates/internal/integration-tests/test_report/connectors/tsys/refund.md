# Connector `tsys` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `66.7%` (`2` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund \| Full Amount`](./refund/refund-full-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Refund \| Partial Amount`](./refund/refund-partial-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Reason`](./refund/refund-with-reason.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Refund | Full Amount`](./refund/refund-full-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist