# Connector `paypal` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`setup_recurring`](./setup-recurring/setup-recurring.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (PASS) |
