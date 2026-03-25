# Connector `adyen` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `33.3%` (`1` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`setup_recurring`](./setup-recurring/setup-recurring.md) | card | credit | `PASS` | None |
| [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | None |
| [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) — Resolved method descriptor:
- [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) — Resolved method descriptor:
