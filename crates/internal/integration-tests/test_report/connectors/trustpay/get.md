# Connector `trustpay` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Get \| Sync Payment`](./get/sync-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`Get \| Sync Payment With Handle Response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Get | Sync Payment`](./get/sync-payment.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"1","message":"unspecified error","reason":"the provided instance id has invalid format & unspecified error"}}
- [`Get | Sync Payment With Handle Response`](./get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist