# Connector `xendit` / Suite `capture`

- Service: `PaymentService/Capture`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Capture \| Full Amount`](./capture/capture-full-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Capture \| Partial Amount`](./capture/capture-partial-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Capture \| Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (PASS) |

## Failed Scenarios

- [`Capture | Full Amount`](./capture/capture-full-amount.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Capture | Partial Amount`](./capture/capture-partial-amount.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Capture | Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) — assertion failed for field 'connector_transaction_id': expected field to exist