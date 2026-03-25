# Connector `powertranz` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `100.0%` (`2` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_sync`](./refund-sync/refund-sync.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) -> `refund(refund_full_amount)` (PASS) |
| [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) -> `refund(refund_full_amount)` (PASS) |
