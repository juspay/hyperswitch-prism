# Connector `iatapay` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `0.0%` (`0` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | None |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | None |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | None |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | None |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | None |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | None |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `FAIL` | None |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `FAIL` | None |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `FAIL` | None |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | None |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `SKIP` | None |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | None |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | None |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | None |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | None |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | None |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `FAIL` | None |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `FAIL` | None |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`ACH Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Affirm | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Afterpay/Clearpay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Alipay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Bancontact | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Credit Card | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Debit Card | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`EPS | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Giropay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — GPAY_HOSTED_URL not set
- [`iDEAL | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Klarna | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Przelewy24 | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Payment Failure | No 3DS`](./authorize/no3ds-fail-payment.md) — assertion failed for field 'error.connector_details.message': expected 'Failed to obtain authentication type' to contain 'decline'
- [`Credit Card | No 3DS | Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Debit Card | No 3DS | Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Credit Card | 3DS | Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist