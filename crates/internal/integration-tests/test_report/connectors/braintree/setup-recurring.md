# Connector `braintree` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Setup Recurring`](./setup-recurring/setup-recurring.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Setup Recurring \| Order Context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Setup Recurring \| Webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |

## Failed Scenarios

- [`Setup Recurring`](./setup-recurring/setup-recurring.md) — sdk call failed: sdk request transformer failed for 'setup_recurring/setup_recurring': Failed to obtain authentication type (code: INTERNAL_SERVER_ERROR)
- [`Setup Recurring | Order Context`](./setup-recurring/setup-recurring-with-order-context.md) — sdk call failed: sdk request transformer failed for 'setup_recurring/setup_recurring_with_order_context': Failed to obtain authentication type (code: INTERNAL_SERVER_ERROR)
- [`Setup Recurring | Webhook`](./setup-recurring/setup-recurring-with-webhook.md) — sdk call failed: sdk request transformer failed for 'setup_recurring/setup_recurring_with_webhook': Failed to obtain authentication type (code: INTERNAL_SERVER_ERROR)