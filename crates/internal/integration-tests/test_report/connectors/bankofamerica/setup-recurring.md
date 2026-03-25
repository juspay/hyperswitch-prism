# Connector `bankofamerica` / Suite `setup_recurring`

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

- [`setup_recurring`](./setup-recurring/setup-recurring.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"MISSING_FIELD","message":"MISSING_FIELD","reason":"Declined - The request is missing one or more fields, detailed_error_information: orderInformation.billTo.locality : MISSING_FIELD, orderInformation.billTo.lastName : MISSING_FIELD, orderInformation.billTo.email : MISSING_FIELD, orderInformation.billTo.address1 : MISSING_FIELD, orderInformation.billTo.country : MISSING_FIELD"}}
- [`setup_recurring_with_order_context`](./setup-recurring/setup-recurring-with-order-context.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"MISSING_FIELD","message":"MISSING_FIELD","reason":"Declined - The request is missing one or more fields, detailed_error_information: orderInformation.billTo.locality : MISSING_FIELD, orderInformation.billTo.lastName : MISSING_FIELD, orderInformation.billTo.email : MISSING_FIELD, orderInformation.billTo.address1 : MISSING_FIELD, orderInformation.billTo.country : MISSING_FIELD"}}
- [`setup_recurring_with_webhook`](./setup-recurring/setup-recurring-with-webhook.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"MISSING_FIELD","message":"MISSING_FIELD","reason":"Declined - The request is missing one or more fields, detailed_error_information: orderInformation.billTo.locality : MISSING_FIELD, orderInformation.billTo.lastName : MISSING_FIELD, orderInformation.billTo.email : MISSING_FIELD, orderInformation.billTo.address1 : MISSING_FIELD, orderInformation.billTo.country : MISSING_FIELD"}}
