# Connector `stripe` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `100.0%` (`5` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`no3ds_auto_capture_credit_card`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`no3ds_auto_capture_debit_card`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`no3ds_fail_payment`](./authorize/no3ds-fail-payment.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`no3ds_manual_capture_credit_card`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`no3ds_manual_capture_debit_card`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | `create_customer(create_customer)` (PASS) |
