# Connector `multisafepay` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund Sync`](./refund-sync/refund-sync.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |
| [`Refund Sync \| Reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |

## Failed Scenarios

- [`Refund Sync`](./refund-sync/refund-sync.md) — Resolved method descriptor:
- [`Refund Sync | Reason`](./refund-sync/refund-sync-with-reason.md) — Resolved method descriptor: