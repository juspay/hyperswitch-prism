# Connector `checkout` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`setup_recurring`](./setup-recurring/setup-recurring.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |

## Failed Scenarios

- [`setup_recurring`](./setup-recurring/setup-recurring.md) — assertion failed for field 'mandate_reference.connector_mandate_id.connector_mandate_id': expected field to exist
- [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) — assertion failed for field 'mandate_reference.connector_mandate_id.connector_mandate_id': expected field to exist
- [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) — assertion failed for field 'mandate_reference.connector_mandate_id.connector_mandate_id': expected field to exist
