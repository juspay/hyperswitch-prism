# Connector `hipay` / Suite `tokenize_payment_method`

- Service: `Unknown`
- Pass Rate: `40.0%` (`2` / `5`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`tokenize_credit_card`](./tokenize-payment-method/tokenize-credit-card.md) | card | - | `FAIL` | None |
| [`tokenize_debit_card`](./tokenize-payment-method/tokenize-debit-card.md) | card | - | `FAIL` | None |
| [`tokenize_fail_expired_card`](./tokenize-payment-method/tokenize-fail-expired-card.md) | card | - | `PASS` | None |
| [`tokenize_fail_invalid_card_number`](./tokenize-payment-method/tokenize-fail-invalid-card-number.md) | card | - | `PASS` | None |
| [`tokenize_with_metadata`](./tokenize-payment-method/tokenize-with-metadata.md) | card | - | `FAIL` | None |

## Failed Scenarios

- [`tokenize_credit_card`](./tokenize-payment-method/tokenize-credit-card.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"400","message":"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">\n<html><head>\n<title>400 Bad Request</title>\n</head><body>\n<h1>Bad Request</h1>\n<p>Your browser sent a request...","reason":"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">\n<html><head>\n<title>400 Bad Request</title>\n</head><body>\n<h1>Bad Request</h1>\n<p>Your browser sent a request..."}}
- [`tokenize_debit_card`](./tokenize-payment-method/tokenize-debit-card.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"400","message":"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">\n<html><head>\n<title>400 Bad Request</title>\n</head><body>\n<h1>Bad Request</h1>\n<p>Your browser sent a request...","reason":"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">\n<html><head>\n<title>400 Bad Request</title>\n</head><body>\n<h1>Bad Request</h1>\n<p>Your browser sent a request..."}}
- [`tokenize_with_metadata`](./tokenize-payment-method/tokenize-with-metadata.md) — assertion failed for field 'error': expected field to be absent or null, got {"connectorDetails":{"code":"400","message":"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">\n<html><head>\n<title>400 Bad Request</title>\n</head><body>\n<h1>Bad Request</h1>\n<p>Your browser sent a request...","reason":"<!DOCTYPE HTML PUBLIC \"-//W3C//DTD HTML 4.01//EN\" \"http://www.w3.org/TR/html4/strict.dtd\">\n<html><head>\n<title>400 Bad Request</title>\n</head><body>\n<h1>Bad Request</h1>\n<p>Your browser sent a request..."}}