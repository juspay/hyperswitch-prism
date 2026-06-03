# Connector `stripe` / Suite `PaymentMethodService/Tokenize`

- Service: `Unknown`
- Pass Rate: `60.0%` (`3` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Tokenize Payment Method \| Tokenize Credit Card`](./paymentmethodservice-tokenize/tokenize-credit-card.md) | card | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize Debit Card`](./paymentmethodservice-tokenize/tokenize-debit-card.md) | card | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize Fail Expired Card`](./paymentmethodservice-tokenize/tokenize-fail-expired-card.md) | card | - | `FAIL` | `CustomerService/Create(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize Fail Invalid Card Number`](./paymentmethodservice-tokenize/tokenize-fail-invalid-card-number.md) | card | - | `FAIL` | `CustomerService/Create(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize With Metadata`](./paymentmethodservice-tokenize/tokenize-with-metadata.md) | card | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) |

## Failed Scenarios

- [`Tokenize Payment Method | Tokenize Fail Expired Card`](./paymentmethodservice-tokenize/tokenize-fail-expired-card.md) — Resolved method descriptor:
- [`Tokenize Payment Method | Tokenize Fail Invalid Card Number`](./paymentmethodservice-tokenize/tokenize-fail-invalid-card-number.md) — Resolved method descriptor:
