# Connector `worldpayvantiv` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `94.7%` (`18` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`no3ds_auto_capture_ach_bank_transfer`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `PASS` | None |
| [`no3ds_auto_capture_affirm`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `PASS` | None |
| [`no3ds_auto_capture_afterpay_clearpay`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `PASS` | None |
| [`no3ds_auto_capture_alipay`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `PASS` | None |
| [`no3ds_auto_capture_bacs_bank_transfer`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `PASS` | None |
| [`no3ds_auto_capture_bancontact`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `PASS` | None |
| [`no3ds_auto_capture_credit_card`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | None |
| [`no3ds_auto_capture_debit_card`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | None |
| [`no3ds_auto_capture_eps`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `PASS` | None |
| [`no3ds_auto_capture_giropay`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `PASS` | None |
| [`no3ds_auto_capture_google_pay_encrypted`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `FAIL` | None |
| [`no3ds_auto_capture_ideal`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `PASS` | None |
| [`no3ds_auto_capture_klarna`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `PASS` | None |
| [`no3ds_auto_capture_przelewy24`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `PASS` | None |
| [`no3ds_auto_capture_sepa_bank_transfer`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `PASS` | None |
| [`no3ds_fail_payment`](./authorize/no3ds-fail-payment.md) | card | credit | `PASS` | None |
| [`no3ds_manual_capture_credit_card`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | None |
| [`no3ds_manual_capture_debit_card`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | None |
| [`threeds_manual_capture_credit_card`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | None |

## Failed Scenarios

- [`no3ds_auto_capture_google_pay_encrypted`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — grpcurl execution failed: [google_pay_token_gen] worldpayvantiv/authorize/no3ds_auto_capture_google_pay_encrypted: npm exited with exit status: 1
