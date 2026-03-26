# Connector `paypal` / Suite `recurring_charge`

- Service: `RecurringPaymentService/Charge`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Recurring Charge`](./recurring-charge/recurring-charge.md) | - | - | `PASS` | `create_access_token(create_access_token)` (PASS) -> `setup_recurring(setup_recurring)` (PASS) |
| [`Recurring Charge \| Low Amount`](./recurring-charge/recurring-charge-low-amount.md) | - | - | `PASS` | `create_access_token(create_access_token)` (PASS) -> `setup_recurring(setup_recurring)` (PASS) |
| [`Recurring Charge \| Order Context`](./recurring-charge/recurring-charge-with-order-context.md) | - | - | `PASS` | `create_access_token(create_access_token)` (PASS) -> `setup_recurring(setup_recurring)` (PASS) |