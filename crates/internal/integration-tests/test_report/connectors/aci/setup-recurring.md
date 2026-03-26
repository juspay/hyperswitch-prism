# Connector `aci` / Suite `setup_recurring`

- Service: `PaymentService/SetupRecurring`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Setup Recurring`](./setup-recurring/setup-recurring.md) | card | credit | `FAIL` | None |
| [`Setup Recurring \| Order Context`](./setup-recurring/setup-recurring-with-order-context.md) | card | credit | `FAIL` | None |
| [`Setup Recurring \| Webhook`](./setup-recurring/setup-recurring-with-webhook.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`Setup Recurring`](./setup-recurring/setup-recurring.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"800.900.300","message":"invalid authentication information"}}
- [`Setup Recurring | Order Context`](./setup-recurring/setup-recurring-with-order-context.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"800.900.300","message":"invalid authentication information"}}
- [`Setup Recurring | Webhook`](./setup-recurring/setup-recurring-with-webhook.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"800.900.300","message":"invalid authentication information"}}