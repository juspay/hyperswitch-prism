# Connector `jpmorgan` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_sync`](./refund-sync/refund-sync.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |
| [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |

## Failed Scenarios

- [`refund_sync`](./refund-sync/refund-sync.md) — Resolved method descriptor:
- [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) — Resolved method descriptor:
