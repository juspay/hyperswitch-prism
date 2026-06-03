# Connector `stripe` / Suite `RecurringPaymentService/Charge`

- Service: `RecurringPaymentService/Charge`
- Pass Rate: `100.0%` (`4` / `4`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Recurring Charge`](./recurringpaymentservice-charge/recurringpaymentservice-charge.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentService/SetupRecurring(setup_recurring)` (PASS) |
| [`Recurring Charge`](./recurringpaymentservice-charge/recurring-charge.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentService/SetupRecurring(setup_recurring)` (PASS) |
| [`Recurring Charge \| Low Amount`](./recurringpaymentservice-charge/recurring-charge-low-amount.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentService/SetupRecurring(setup_recurring)` (PASS) |
| [`Recurring Charge \| Order Context`](./recurringpaymentservice-charge/recurring-charge-with-order-context.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentService/SetupRecurring(setup_recurring)` (PASS) |
