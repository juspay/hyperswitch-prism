# Connector `helcim` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Get \| Sync Payment`](./get/sync-payment.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Get \| Sync Payment With Handle Response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Get | Sync Payment`](./get/sync-payment.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"No error code","message":"Failed to retrieve card transaction #auto_generate.","reason":"Failed to retrieve card transaction #auto_generate."}}
- [`Get | Sync Payment With Handle Response`](./get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist