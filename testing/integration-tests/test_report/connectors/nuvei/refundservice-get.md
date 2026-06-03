# Connector `nuvei` / Suite `RefundService/Get`

- Service: `RefundService/Get`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Refund Sync`](./refundservice-get/refundservice-get.md) | - | - | `FAIL` | `MerchantAuthenticationService/CreateServerSessionAuthenticationToken(create_session_basic)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) -> `PaymentService/Refund(refund_full_amount)` (PASS) |
| [`Refund Sync`](./refundservice-get/refund-sync.md) | - | - | `FAIL` | `MerchantAuthenticationService/CreateServerSessionAuthenticationToken(create_session_basic)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) -> `PaymentService/Refund(refund_full_amount)` (PASS) |
| [`Refund Sync \| Reason`](./refundservice-get/refund-sync-with-reason.md) | - | - | `FAIL` | `MerchantAuthenticationService/CreateServerSessionAuthenticationToken(create_session_basic)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) -> `PaymentService/Refund(refund_full_amount)` (PASS) |

## Failed Scenarios

- [`Refund Sync`](./refundservice-get/refundservice-get.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"9146","message":"No transaction details returned for the provided id.","reason":"No transaction details returned for the provided id."}}
- [`Refund Sync`](./refundservice-get/refund-sync.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"9146","message":"No transaction details returned for the provided id.","reason":"No transaction details returned for the provided id."}}
- [`Refund Sync | Reason`](./refundservice-get/refund-sync-with-reason.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"9146","message":"No transaction details returned for the provided id.","reason":"No transaction details returned for the provided id."}}
