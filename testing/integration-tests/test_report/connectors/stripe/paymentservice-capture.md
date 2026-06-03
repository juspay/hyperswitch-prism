# Connector `stripe` / Suite `PaymentService/Capture`

- Service: `PaymentService/Capture`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Capture \| Full Amount`](./paymentservice-capture/capture-full-amount.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Capture \| Partial Amount`](./paymentservice-capture/capture-partial-amount.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Capture \| Merchant Order ID Reference`](./paymentservice-capture/capture-with-merchant-order-id.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
