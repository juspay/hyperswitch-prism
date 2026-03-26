# Connector `stax` / Suite `tokenize_payment_method`

- Service: `Unknown`
- Pass Rate: `40.0%` (`2` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`Tokenize Payment Method \| Tokenize Credit Card`](./tokenize-payment-method/tokenize-credit-card.md) | card | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize Debit Card`](./tokenize-payment-method/tokenize-debit-card.md) | card | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize Fail Expired Card`](./tokenize-payment-method/tokenize-fail-expired-card.md) | card | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize Fail Invalid Card Number`](./tokenize-payment-method/tokenize-fail-invalid-card-number.md) | card | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Tokenize Payment Method \| Tokenize With Metadata`](./tokenize-payment-method/tokenize-with-metadata.md) | card | - | `FAIL` | `create_customer(create_customer)` (PASS) |

## Failed Scenarios

- [`Tokenize Payment Method | Tokenize Credit Card`](./tokenize-payment-method/tokenize-credit-card.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"422","message":"The selected customer id is invalid.","reason":"{\"customer_id\":[\"The selected customer id is invalid.\"]}"}}
- [`Tokenize Payment Method | Tokenize Debit Card`](./tokenize-payment-method/tokenize-debit-card.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"422","message":"The selected customer id is invalid.","reason":"{\"customer_id\":[\"The selected customer id is invalid.\"]}"}}
- [`Tokenize Payment Method | Tokenize With Metadata`](./tokenize-payment-method/tokenize-with-metadata.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"422","message":"The selected customer id is invalid.","reason":"{\"customer_id\":[\"The selected customer id is invalid.\"]}"}}