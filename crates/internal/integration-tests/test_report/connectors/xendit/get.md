# Connector `xendit` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `50.0%` (`1` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist
