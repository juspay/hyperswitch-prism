# Connector `stripe` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Setup Recurring`](./setup-recurring/setup-recurring.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Setup Recurring \| Order Context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Setup Recurring \| Webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |