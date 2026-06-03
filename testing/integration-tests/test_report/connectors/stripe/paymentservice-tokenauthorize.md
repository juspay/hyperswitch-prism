# Connector `stripe` / Suite `PaymentService/TokenAuthorize`

- Service: `Unknown`
- Pass Rate: `0.0%` (`0` / `2`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Saved Token \| Auto Capture`](./paymentservice-tokenauthorize/token-auto-capture-credit-card.md) | - | - | `FAIL` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Saved Token \| Manual Capture`](./paymentservice-tokenauthorize/token-manual-capture-credit-card.md) | - | - | `FAIL` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |

## Failed Scenarios

- [`Saved Token | Auto Capture`](./paymentservice-tokenauthorize/token-auto-capture-credit-card.md) — Resolved method descriptor:
- [`Saved Token | Manual Capture`](./paymentservice-tokenauthorize/token-manual-capture-credit-card.md) — Resolved method descriptor:
