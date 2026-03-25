# Connector `cybersource` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `33.3%` (`1` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`void_authorized_payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_with_amount`](./void/void-with-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`void_authorized_payment`](./void/void-authorized-payment.md) — sdk call failed: sdk request transformer failed for 'void/void_authorized_payment': Missing required field: currency (code: BAD_REQUEST)
- [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) — sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Missing required field: currency (code: BAD_REQUEST)
