# Connector `braintree` / Suite `tokenize_payment_method`

- Service: `Unknown`
- Pass Rate: `80.0%` (`4` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`tokenize_credit_card`](./tokenize-payment-method/tokenize-credit-card.md) | card | - | `PASS` | None |
| [`tokenize_debit_card`](./tokenize-payment-method/tokenize-debit-card.md) | card | - | `PASS` | None |
| [`tokenize_fail_expired_card`](./tokenize-payment-method/tokenize-fail-expired-card.md) | card | - | `FAIL` | None |
| [`tokenize_fail_invalid_card_number`](./tokenize-payment-method/tokenize-fail-invalid-card-number.md) | card | - | `PASS` | None |
| [`tokenize_with_metadata`](./tokenize-payment-method/tokenize-with-metadata.md) | card | - | `PASS` | None |

## Failed Scenarios

- [`tokenize_fail_expired_card`](./tokenize-payment-method/tokenize-fail-expired-card.md) — assertion failed for field 'error': expected field to exist
