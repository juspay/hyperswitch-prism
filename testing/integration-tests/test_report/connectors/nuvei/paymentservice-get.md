# Connector `nuvei` / Suite `PaymentService/Get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Get \| Sync Payment`](./paymentservice-get/sync-payment.md) | - | - | `FAIL` | `MerchantAuthenticationService/CreateServerSessionAuthenticationToken(create_session_basic)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Get \| Sync Payment With Handle Response`](./paymentservice-get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `MerchantAuthenticationService/CreateServerSessionAuthenticationToken(create_session_basic)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Get | Sync Payment`](./paymentservice-get/sync-payment.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"9146","message":"No transaction details returned for the provided id.","reason":"No transaction details returned for the provided id."}}
- [`Get | Sync Payment With Handle Response`](./paymentservice-get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist
