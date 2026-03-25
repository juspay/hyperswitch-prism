# Connector `bankofamerica` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_sync`](./refund-sync/refund-sync.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) -> `refund(refund_full_amount)` (PASS) |
| [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) -> `refund(refund_full_amount)` (PASS) |

## Failed Scenarios

- [`refund_sync`](./refund-sync/refund-sync.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"No error code","message":"No error message","reason":"The requested resource does not exist"}}
- [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"No error code","message":"No error message","reason":"The requested resource does not exist"}}
