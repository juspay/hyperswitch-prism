# Connector `rapyd` / Suite `RefundService/Get`

- Service: `RefundService/Get`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund Sync`](./refundservice-get/refundservice-get.md) | - | - | `PASS` | `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) -> `PaymentService/Refund(refund_full_amount)` (PASS) |
| [`Refund Sync`](./refundservice-get/refund-sync.md) | - | - | `PASS` | `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) -> `PaymentService/Refund(refund_full_amount)` (PASS) |
| [`Refund Sync \| Reason`](./refundservice-get/refund-sync-with-reason.md) | - | - | `PASS` | `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) -> `PaymentService/Refund(refund_full_amount)` (PASS) |
