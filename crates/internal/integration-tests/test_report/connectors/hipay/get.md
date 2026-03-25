# Connector `hipay` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `FAIL` | `authorize(no3ds_auto_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`sync_payment`](./get/sync-payment.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"404","message":"{\"message\":\"No route found for \\\"GET https:\\/\\/stage-api-gateway.hipay.com\\/v3\\/transaction\\/auto_generate\\\"\",\"code\":0}","reason":"{\"message\":\"No route found for \\\"GET https:\\/\\/stage-api-gateway.hipay.com\\/v3\\/transaction\\/auto_generate\\\"\",\"code\":0}"}}
- [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) — assertion failed for field 'connector_transaction_id': expected field to exist
