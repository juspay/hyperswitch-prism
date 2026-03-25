# Connector `cybersource` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_sync`](./refund-sync/refund-sync.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`refund_sync`](./refund-sync/refund-sync.md) — unsupported suite 'refund_sync' for grpcurl generation
- [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) — unsupported suite 'refund_sync' for grpcurl generation
