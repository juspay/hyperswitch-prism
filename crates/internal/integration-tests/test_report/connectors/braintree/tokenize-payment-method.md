# Connector `braintree` / Suite `tokenize_payment_method`

- Service: `Unknown`
- Pass Rate: `80.0%` (`4` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Tokenize Payment Method \| Tokenize Credit Card`](./tokenize-payment-method/tokenize-credit-card.md) | card | - | `PASS` | None |
| [`Tokenize Payment Method \| Tokenize Debit Card`](./tokenize-payment-method/tokenize-debit-card.md) | card | - | `PASS` | None |
| [`Tokenize Payment Method \| Tokenize Fail Expired Card`](./tokenize-payment-method/tokenize-fail-expired-card.md) | card | - | `FAIL` | None |
| [`Tokenize Payment Method \| Tokenize Fail Invalid Card Number`](./tokenize-payment-method/tokenize-fail-invalid-card-number.md) | card | - | `PASS` | None |
| [`Tokenize Payment Method \| Tokenize With Metadata`](./tokenize-payment-method/tokenize-with-metadata.md) | card | - | `PASS` | None |

## Failed Scenarios

- [`Tokenize Payment Method | Tokenize Fail Expired Card`](./tokenize-payment-method/tokenize-fail-expired-card.md) — assertion failed for field 'error': expected field to exist