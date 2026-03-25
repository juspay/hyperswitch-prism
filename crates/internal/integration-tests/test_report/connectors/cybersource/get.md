# Connector `cybersource` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`sync_payment`](./get/sync-payment.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuer_details":{"network_details":{}},"connector_details":{"code":"No error code","message":"The requested resource does not exist","reason":"The requested resource does not exist"}}
- [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist
