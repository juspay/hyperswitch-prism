# Connector `payload` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `50.0%` (`1` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_sync`](./refund-sync/refund-sync.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) -> `refund(refund_full_amount)` (PASS) |
| [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |

## Failed Scenarios

- [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) — Resolved method descriptor:
