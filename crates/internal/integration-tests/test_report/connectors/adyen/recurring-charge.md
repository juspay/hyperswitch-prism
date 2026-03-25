# Connector `adyen` / Suite `recurring_charge`

- Service: `RecurringPaymentService/Charge`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`recurring_charge`](./recurring-charge/recurring-charge.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (PASS) |
| [`recurring_charge_low_amount`](./recurring-charge/recurring-charge-low-amount.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (PASS) |
| [`recurring_charge_with_order_context`](./recurring-charge/recurring-charge-with-order-context.md) | - | - | `FAIL` | `setup_recurring(setup_recurring)` (PASS) |

## Failed Scenarios

- [`recurring_charge`](./recurring-charge/recurring-charge.md) — Resolved method descriptor:
- [`recurring_charge_low_amount`](./recurring-charge/recurring-charge-low-amount.md) — Resolved method descriptor:
- [`recurring_charge_with_order_context`](./recurring-charge/recurring-charge-with-order-context.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"121","message":"Required field 'shopperReference' is not provided.","reason":"Required field 'shopperReference' is not provided."}}
