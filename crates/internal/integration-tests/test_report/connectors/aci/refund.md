# Connector `aci` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund \| Full Amount`](./refund/refund-full-amount.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Refund \| Partial Amount`](./refund/refund-partial-amount.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Refund \| Reason`](./refund/refund-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Refund | Full Amount`](./refund/refund-full-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist
- [`Refund | Partial Amount`](./refund/refund-partial-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist
- [`Refund | Reason`](./refund/refund-with-reason.md) — assertion failed for field 'connector_refund_id': expected field to exist