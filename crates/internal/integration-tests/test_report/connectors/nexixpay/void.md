# Connector `nexixpay` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./void/void-authorized-payment.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Void \| Amount`](./void/void-with-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Void \| Without Cancellation Reason`](./void/void-without-cancellation-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Void | Authorized Payment`](./void/void-authorized-payment.md) — sdk call failed: sdk request transformer failed for 'void/void_authorized_payment': Missing required field: amount for void operation (code: BAD_REQUEST)
- [`Void | Amount`](./void/void-with-amount.md) — sdk call failed: sdk response transformer failed for 'void/void_with_amount': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
- [`Void | Without Cancellation Reason`](./void/void-without-cancellation-reason.md) — sdk call failed: sdk request transformer failed for 'void/void_without_cancellation_reason': Missing required field: amount for void operation (code: BAD_REQUEST)