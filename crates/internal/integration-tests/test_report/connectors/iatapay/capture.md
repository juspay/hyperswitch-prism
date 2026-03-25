# Connector `iatapay` / Suite `capture`

- Service: `PaymentService/Capture`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`capture_full_amount`](./capture/capture-full-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`capture_partial_amount`](./capture/capture-partial-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`capture_with_merchant_order_id`](./capture/capture-with-merchant-order-id.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`capture_full_amount`](./capture/capture-full-amount.md) — Resolved method descriptor:
- [`capture_partial_amount`](./capture/capture-partial-amount.md) — Resolved method descriptor:
- [`capture_with_merchant_order_id`](./capture/capture-with-merchant-order-id.md) — Resolved method descriptor:
