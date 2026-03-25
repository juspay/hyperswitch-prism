# Connector `bluesnap` / Suite `void`

- Service: `PaymentService/Void`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`void_authorized_payment`](./void/void-authorized-payment.md) | - | - | `PASS` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_with_amount`](./void/void-with-amount.md) | - | - | `PASS` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`void_without_cancellation_reason`](./void/void-without-cancellation-reason.md) | - | - | `PASS` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
