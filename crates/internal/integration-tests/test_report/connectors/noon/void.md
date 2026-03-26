# Connector `noon` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Void \| Amount`](./void/void-with-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Void \| Without Cancellation Reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Void | Authorized Payment`](./void/void-authorized-payment.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Void | Amount`](./void/void-with-amount.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Void | Without Cancellation Reason`](./void/void-without-cancellation-reason.md) — assertion failed for field 'connector_transaction_id': expected field to exist