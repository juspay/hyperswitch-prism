# Connector `cybersource` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `33.3%` (`1` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Amount`](./void/void-with-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Without Cancellation Reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Void | Authorized Payment`](./void/void-authorized-payment.md) — sdk call failed: sdk request transformer failed for 'void/void_authorized_payment': Missing required field: currency (code: BAD_REQUEST)
- [`Void | Without Cancellation Reason`](./void/void-without-cancellation-reason.md) — sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Missing required field: currency (code: BAD_REQUEST)