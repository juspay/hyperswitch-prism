# Connector `stripe` / Suite `PaymentService/SetupRecurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `100.0%` (`4` / `4`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Setup Recurring`](./paymentservice-setuprecurring/paymentservice-setuprecurring.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) |
| [`Setup Recurring`](./paymentservice-setuprecurring/setup-recurring.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) |
| [`Setup Recurring \| Order Context`](./paymentservice-setuprecurring/setup-recurring-with-order-context.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) |
| [`Setup Recurring \| Webhook`](./paymentservice-setuprecurring/setup-recurring-with-webhook.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) |
