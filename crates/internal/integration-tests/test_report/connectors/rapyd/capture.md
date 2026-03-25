# Connector `rapyd` / Suite `capture`

- Service: `PaymentService/Capture`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`capture_full_amount`](./capture/capture-full-amount.md) | - | - | `PASS` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`capture_partial_amount`](./capture/capture-partial-amount.md) | - | - | `PASS` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`capture_with_merchant_order_id`](./capture/capture-with-merchant-order-id.md) | - | - | `PASS` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
