# Connector `authorizedotnet` / Suite `capture`

- Service: `PaymentService/Capture`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Capture \| Full Amount`](./capture/capture-full-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Capture \| Partial Amount`](./capture/capture-partial-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Capture \| Merchant Order ID Reference`](./capture/capture-with-merchant-order-id.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `authorize(no3ds_manual_capture_credit_card)` (PASS) |