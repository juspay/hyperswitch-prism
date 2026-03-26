# Connector `xendit` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `33.3%` (`1` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund \| Full Amount`](./refund/refund-full-amount.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Partial Amount`](./refund/refund-partial-amount.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund \| Reason`](./refund/refund-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Refund | Partial Amount`](./refund/refund-partial-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist
- [`Refund | Reason`](./refund/refund-with-reason.md) — assertion failed for field 'connector_refund_id': expected field to exist