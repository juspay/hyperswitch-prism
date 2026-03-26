# Connector `nexixpay` / Suite `capture`

- Service: `PaymentService/Capture`
- Pass Rate: `0.0%` (`0` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Capture \| Full Amount`](./capture/capture-full-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Capture \| Partial Amount`](./capture/capture-partial-amount.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |
| [`Capture \| Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) | - | - | `FAIL` | `create_customer(create_customer)` (FAIL) -> `authorize(no3ds_manual_capture_credit_card)` (FAIL) |

## Failed Scenarios

- [`Capture | Full Amount`](./capture/capture-full-amount.md) — sdk call failed: sdk response transformer failed for 'capture/capture_full_amount': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
- [`Capture | Partial Amount`](./capture/capture-partial-amount.md) — sdk call failed: sdk response transformer failed for 'capture/capture_partial_amount': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)
- [`Capture | Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) — sdk call failed: sdk response transformer failed for 'capture/capture_with_merchant_order_id': Failed to deserialize connector response (code: INTERNAL_SERVER_ERROR)