# Connector `payload` / Suite `recurring_charge`

- Service: `RecurringPaymentService/Charge`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Recurring Charge`](./recurring-charge/recurring-charge.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (FAIL) |
| [`Recurring Charge \| Low Amount`](./recurring-charge/recurring-charge-low-amount.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (FAIL) |
| [`Recurring Charge \| Order Context`](./recurring-charge/recurring-charge-with-order-context.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (FAIL) |

## Failed Scenarios

- [`Recurring Charge`](./recurring-charge/recurring-charge.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Recurring Charge | Low Amount`](./recurring-charge/recurring-charge-low-amount.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Recurring Charge | Order Context`](./recurring-charge/recurring-charge-with-order-context.md) — assertion failed for field 'connector_transaction_id': expected field to exist