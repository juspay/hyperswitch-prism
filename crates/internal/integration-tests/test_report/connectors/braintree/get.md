# Connector `braintree` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`sync_payment`](./get/sync-payment.md) — Resolved method descriptor:
- [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) — Resolved method descriptor:
