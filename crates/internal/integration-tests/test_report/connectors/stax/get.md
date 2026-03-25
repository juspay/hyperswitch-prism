# Connector `stax` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`sync_payment`](./get/sync-payment.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"422","message":"The selected id is invalid.","reason":"{\"id\":[\"The selected id is invalid.\"]}"}}
- [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist
