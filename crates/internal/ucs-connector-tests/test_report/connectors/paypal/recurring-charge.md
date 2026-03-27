# Connector `paypal` / Suite `recurring_charge`

- Service: `RecurringPaymentService/Charge`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`recurring_charge`](./recurring-charge/recurring-charge.md) | - | - | `PASS` | `create_access_token(create_access_token)` (PASS) -> `setup_recurring(setup_recurring)` (PASS) |
| [`recurring_charge_low_amount`](./recurring-charge/recurring-charge-low-amount.md) | - | - | `PASS` | `create_access_token(create_access_token)` (PASS) -> `setup_recurring(setup_recurring)` (PASS) |
| [`recurring_charge_with_order_context`](./recurring-charge/recurring-charge-with-order-context.md) | - | - | `PASS` | `create_access_token(create_access_token)` (PASS) -> `setup_recurring(setup_recurring)` (PASS) |
