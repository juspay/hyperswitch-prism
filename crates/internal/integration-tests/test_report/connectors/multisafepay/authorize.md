# Connector `multisafepay` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `21.1%` (`4` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `PASS` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `PASS` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `PASS` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `FAIL` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (FAIL) -> `create_customer(create_customer)` (FAIL) -> `tokenize_payment_method(tokenize_credit_card)` (FAIL) |

## Failed Scenarios

- [`ACH Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Affirm | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Afterpay/Clearpay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Bancontact | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Credit Card | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) — assertion failed for field 'status': expected one of ["CHARGED", "AUTHORIZED"], got "AUTHENTICATION_PENDING"
- [`Debit Card | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) — assertion failed for field 'status': expected one of ["CHARGED", "AUTHORIZED"], got "AUTHENTICATION_PENDING"
- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — grpcurl execution failed: [google_pay_token_gen] multisafepay/authorize/no3ds_auto_capture_google_pay_encrypted: npm exited with exit status: 1
- [`iDEAL | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Klarna | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Przelewy24 | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Payment Failure | No 3DS`](./authorize/no3ds-fail-payment.md) — assertion failed for field 'error': expected field to exist
- [`Credit Card | No 3DS | Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) — assertion failed for field 'status': expected one of ["AUTHORIZED"], got "AUTHENTICATION_PENDING"
- [`Debit Card | No 3DS | Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) — assertion failed for field 'status': expected one of ["AUTHORIZED"], got "AUTHENTICATION_PENDING"