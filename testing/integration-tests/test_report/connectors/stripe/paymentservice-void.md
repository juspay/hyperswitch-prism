# Connector `stripe` / Suite `PaymentService/Void`

- Service: `PaymentService/Void`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Void \| Authorized Payment`](./paymentservice-void/void-authorized-payment.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Amount`](./paymentservice-void/void-with-amount.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
| [`Void \| Without Cancellation Reason`](./paymentservice-void/void-without-cancellation-reason.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_manual_capture_credit_card)` (PASS) |
