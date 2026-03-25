# Connector `novalnet` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`sync_payment`](./get/sync-payment.md) — sdk call failed: sdk response transformer failed for 'get/sync_payment': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
- [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) — sdk call failed: sdk response transformer failed for 'get/sync_payment_with_handle_response': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
