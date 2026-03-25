# Connector `hipay` / Suite `refund_sync`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_sync`](./refund-sync/refund-sync.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |
| [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) -> `refund(refund_full_amount)` (FAIL) |

## Failed Scenarios

- [`refund_sync`](./refund-sync/refund-sync.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"404","message":"{\"message\":\"No route found for \\\"GET https:\\/\\/stage-api-gateway.hipay.com\\/v3\\/transaction\\/\\\"\",\"code\":0}","reason":"{\"message\":\"No route found for \\\"GET https:\\/\\/stage-api-gateway.hipay.com\\/v3\\/transaction\\/\\\"\",\"code\":0}"}}
- [`refund_sync_with_reason`](./refund-sync/refund-sync-with-reason.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"404","message":"{\"message\":\"No route found for \\\"GET https:\\/\\/stage-api-gateway.hipay.com\\/v3\\/transaction\\/\\\"\",\"code\":0}","reason":"{\"message\":\"No route found for \\\"GET https:\\/\\/stage-api-gateway.hipay.com\\/v3\\/transaction\\/\\\"\",\"code\":0}"}}
