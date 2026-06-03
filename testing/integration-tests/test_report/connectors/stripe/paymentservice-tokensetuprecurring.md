# Connector `stripe` / Suite `PaymentService/TokenSetupRecurring`

- Service: `Unknown`
- Pass Rate: `0.0%` (`0` / `1`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Saved Token \| Setup Mandate`](./paymentservice-tokensetuprecurring/token-setup-mandate.md) | token | - | `FAIL` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |

## Failed Scenarios

- [`Saved Token | Setup Mandate`](./paymentservice-tokensetuprecurring/token-setup-mandate.md) — Resolved method descriptor:
