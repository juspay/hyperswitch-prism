# Connector `stripe` / Suite `refund`

- Service: `PaymentService/Refund`
- Pass Rate: `100.0%` (`3` / `3`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`refund_full_amount`](./refund/refund-full-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`refund_partial_amount`](./refund/refund-partial-amount.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
| [`refund_with_reason`](./refund/refund-with-reason.md) | - | - | `PASS` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) -> `authorize(no3ds_auto_capture_credit_card)` (PASS) |
