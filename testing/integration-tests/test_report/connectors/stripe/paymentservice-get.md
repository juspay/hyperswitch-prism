# Connector `stripe` / Suite `PaymentService/Get`

- Service: `PaymentService/Get`
- Pass Rate: `100.0%` (`2` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Get \| Sync Payment`](./paymentservice-get/sync-payment.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`Get \| Sync Payment With Handle Response`](./paymentservice-get/sync-payment-with-handle-response.md) | - | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) -> `PaymentService/Authorize(no3ds_auto_capture_credit_card)` (PASS) |
