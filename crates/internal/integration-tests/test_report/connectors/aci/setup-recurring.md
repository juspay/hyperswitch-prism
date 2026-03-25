# Connector `aci` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`setup_recurring`](./setup-recurring/setup-recurring.md) | card | credit | `FAIL` | None |
| [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | None |
| [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`setup_recurring`](./setup-recurring/setup-recurring.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"800.900.300","message":"invalid authentication information"}}
- [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"800.900.300","message":"invalid authentication information"}}
- [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"800.900.300","message":"invalid authentication information"}}
