# Connector `paybox` / Suite `capture`

- Service: `PaymentService/Capture`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Capture \| Full Amount`](./capture/capture-full-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Capture \| Partial Amount`](./capture/capture-partial-amount.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Capture \| Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) | - | - | `FAIL` | `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Capture | Full Amount`](./capture/capture-full-amount.md) — Resolved method descriptor:
- [`Capture | Partial Amount`](./capture/capture-partial-amount.md) — Resolved method descriptor:
- [`Capture | Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) — Resolved method descriptor: