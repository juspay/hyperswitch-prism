# Connector `rapyd` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund Sync`](./refund-sync/refund-sync.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Refund Sync \| Reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Refund Sync`](./refund-sync/refund-sync.md) — unsupported suite 'refund_sync' for grpcurl generation
- [`Refund Sync | Reason`](./refund-sync/refund-sync-with-reason.md) — unsupported suite 'refund_sync' for grpcurl generation