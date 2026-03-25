# Connector `checkout` / Suite `get`

- Service: `PaymentService/Get`
- Pass Rate: `100.0%` (`2` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`sync_payment`](./get/sync-payment.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`sync_payment_with_handle_response`](./get/sync-payment-with-handle-response.md) | - | - | `PASS` | `authorize(no3ds_auto_capture_credit_card)` (PASS) |
