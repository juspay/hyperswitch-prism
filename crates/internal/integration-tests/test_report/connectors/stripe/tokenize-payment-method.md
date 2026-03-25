# Connector `stripe` / Suite `tokenize_payment_method`

- Service: `Unknown`
- Pass Rate: `40.0%` (`2` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`tokenize_credit_card`](./tokenize-payment-method/tokenize-credit-card.md) | card | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`tokenize_debit_card`](./tokenize-payment-method/tokenize-debit-card.md) | card | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`tokenize_fail_expired_card`](./tokenize-payment-method/tokenize-fail-expired-card.md) | card | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`tokenize_fail_invalid_card_number`](./tokenize-payment-method/tokenize-fail-invalid-card-number.md) | card | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`tokenize_with_metadata`](./tokenize-payment-method/tokenize-with-metadata.md) | card | - | `FAIL` | `create_customer(create_customer)` (PASS) |

## Failed Scenarios

- [`tokenize_credit_card`](./tokenize-payment-method/tokenize-credit-card.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"parameter_unknown","message":"Received unknown parameters: billing_details, type","reason":"Received unknown parameters: billing_details, type"}}
- [`tokenize_debit_card`](./tokenize-payment-method/tokenize-debit-card.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"parameter_unknown","message":"Received unknown parameters: billing_details, type","reason":"Received unknown parameters: billing_details, type"}}
- [`tokenize_with_metadata`](./tokenize-payment-method/tokenize-with-metadata.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"parameter_unknown","message":"Received unknown parameter: type","reason":"Received unknown parameter: type"}}
