# Connector `stax` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Setup Recurring`](./setup-recurring/setup-recurring.md) | card | credit | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Setup Recurring \| Order Context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Setup Recurring \| Webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | `create_customer(create_customer)` (PASS) |

## Failed Scenarios

- [`Setup Recurring`](./setup-recurring/setup-recurring.md) — Resolved method descriptor:
- [`Setup Recurring | Order Context`](./setup-recurring/setup-recurring-with-order-context.md) — Resolved method descriptor:
- [`Setup Recurring | Webhook`](./setup-recurring/setup-recurring-with-webhook.md) — Resolved method descriptor: