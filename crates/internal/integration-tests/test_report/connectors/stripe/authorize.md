# Connector `stripe` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `84.2%` (`16` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `PASS` | `create_customer(create_customer)` (PASS) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `PASS` | `create_customer(create_customer)` (PASS) |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (PASS) |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | `create_customer(create_customer)` (PASS) |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_customer(create_customer)` (PASS) |

## Failed Scenarios

- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Giropay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist