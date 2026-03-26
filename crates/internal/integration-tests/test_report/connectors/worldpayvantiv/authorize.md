# Connector `worldpayvantiv` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `94.7%` (`18` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `PASS` | None |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `PASS` | None |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `PASS` | None |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `PASS` | None |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `PASS` | None |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `PASS` | None |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | None |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | None |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `PASS` | None |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `PASS` | None |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `FAIL` | None |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `PASS` | None |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `PASS` | None |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `PASS` | None |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `PASS` | None |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `PASS` | None |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | None |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | None |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | None |

## Failed Scenarios

- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — grpcurl execution failed: [google_pay_token_gen] worldpayvantiv/authorize/no3ds_auto_capture_google_pay_encrypted: npm exited with exit status: 1