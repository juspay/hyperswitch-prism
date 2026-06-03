# Connector `rapyd` / Suite `PaymentService/Authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `26.3%` (`5` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | None |
| [`Affirm \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | None |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | None |
| [`Alipay \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | None |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | None |
| [`Bancontact \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | None |
| [`Credit Card \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | None |
| [`Debit Card \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | None |
| [`EPS \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-eps.md) | eps | - | `FAIL` | None |
| [`Giropay \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | None |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `SKIP` | None |
| [`iDEAL \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | None |
| [`Klarna \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | None |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | None |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | None |
| [`Payment Failure \| No 3DS`](./paymentservice-authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | None |
| [`Credit Card \| No 3DS \| Manual Capture`](./paymentservice-authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | None |
| [`Debit Card \| No 3DS \| Manual Capture`](./paymentservice-authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | None |
| [`Credit Card \| 3DS \| Manual Capture`](./paymentservice-authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | None |

## Failed Scenarios

- [`ACH Bank Transfer | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-ach-bank-transfer.md) — Resolved method descriptor:
- [`Affirm | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-affirm.md) — Resolved method descriptor:
- [`Afterpay/Clearpay | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-afterpay-clearpay.md) — Resolved method descriptor:
- [`Alipay | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-alipay.md) — Resolved method descriptor:
- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-bacs-bank-transfer.md) — Resolved method descriptor:
- [`Bancontact | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-bancontact.md) — Resolved method descriptor:
- [`EPS | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-eps.md) — Resolved method descriptor:
- [`Giropay | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-giropay.md) — Resolved method descriptor:
- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-google-pay-encrypted.md) — credentials for connector 'rapyd' do not include metadata.google_pay; add a `metadata.google_pay` block under `rapyd` in '/Users/amitsingh.tanwar/Documents/connector-service/connector-service/creds.json'. Refer to `browser-automation-engine/src/gpay-token-gen.ts` for the expected shape and use any existing connector entry in `creds.json` that already has `metadata.google_pay` as a template
- [`iDEAL | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-ideal.md) — Resolved method descriptor:
- [`Klarna | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-klarna.md) — Resolved method descriptor:
- [`Przelewy24 | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-przelewy24.md) — Resolved method descriptor:
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-sepa-bank-transfer.md) — Resolved method descriptor:
- [`Payment Failure | No 3DS`](./paymentservice-authorize/no3ds-fail-payment.md) — assertion failed for field 'error': expected field to exist
