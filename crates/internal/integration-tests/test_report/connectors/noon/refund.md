# Connector `noon` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_full_amount`](./refund/refund-full-amount.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`refund_partial_amount`](./refund/refund-partial-amount.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`refund_with_reason`](./refund/refund-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`refund_full_amount`](./refund/refund-full-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist
- [`refund_partial_amount`](./refund/refund-partial-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist
- [`refund_with_reason`](./refund/refund-with-reason.md) — assertion failed for field 'connector_refund_id': expected field to exist
