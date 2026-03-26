# Connector `braintree` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `21.1%` (`4` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `SKIP` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `FAIL` | `tokenize_payment_method(tokenize_credit_card)` (PASS) |

## Failed Scenarios

- [`ACH Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Affirm | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Afterpay/Clearpay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Alipay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Bancontact | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`EPS | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Giropay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — GPAY_HOSTED_URL not set
- [`iDEAL | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Klarna | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Przelewy24 | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Payment Failure | No 3DS`](./authorize/no3ds-fail-payment.md) — assertion failed for field 'error': expected field to exist
- [`Credit Card | 3DS | Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist