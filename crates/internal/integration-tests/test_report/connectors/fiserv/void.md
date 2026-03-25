# Connector `fiserv` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`void_authorized_payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`void_with_amount`](./void/void-with-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`void_authorized_payment`](./void/void-authorized-payment.md) — sdk call failed: sdk request transformer failed for 'void/void_authorized_payment': Failed to encode connector request (code: INTERNAL_SERVER_ERROR)
- [`void_with_amount`](./void/void-with-amount.md) — sdk call failed: sdk request transformer failed for 'void/void_with_amount': Failed to encode connector request (code: INTERNAL_SERVER_ERROR)
- [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) — sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Failed to encode connector request (code: INTERNAL_SERVER_ERROR)
