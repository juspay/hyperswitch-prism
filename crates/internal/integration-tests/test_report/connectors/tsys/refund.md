# Connector `tsys` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `66.7%` (`2` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_full_amount`](./refund/refund-full-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`refund_partial_amount`](./refund/refund-partial-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`refund_with_reason`](./refund/refund-with-reason.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`refund_full_amount`](./refund/refund-full-amount.md) — assertion failed for field 'connector_refund_id': expected field to exist
