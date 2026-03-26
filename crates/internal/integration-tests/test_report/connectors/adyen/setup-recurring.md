# Connector `adyen` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `33.3%` (`1` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Setup Recurring`](./setup-recurring/setup-recurring.md) | card | credit | `PASS` | None |
| [`Setup Recurring \| Order Context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | None |
| [`Setup Recurring \| Webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`Setup Recurring | Order Context`](./setup-recurring/setup-recurring-with-order-context.md) — Resolved method descriptor:
- [`Setup Recurring | Webhook`](./setup-recurring/setup-recurring-with-webhook.md) — Resolved method descriptor: