# Connector `payu` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `0.0%` (`0` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`no3ds_auto_capture_ach_bank_transfer`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | None |
| [`no3ds_auto_capture_affirm`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | None |
| [`no3ds_auto_capture_afterpay_clearpay`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | None |
| [`no3ds_auto_capture_alipay`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | None |
| [`no3ds_auto_capture_bacs_bank_transfer`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | None |
| [`no3ds_auto_capture_bancontact`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | None |
| [`no3ds_auto_capture_credit_card`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `FAIL` | None |
| [`no3ds_auto_capture_debit_card`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `FAIL` | None |
| [`no3ds_auto_capture_eps`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `FAIL` | None |
| [`no3ds_auto_capture_giropay`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | None |
| [`no3ds_auto_capture_google_pay_encrypted`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `SKIP` | None |
| [`no3ds_auto_capture_ideal`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | None |
| [`no3ds_auto_capture_klarna`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | None |
| [`no3ds_auto_capture_przelewy24`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | None |
| [`no3ds_auto_capture_sepa_bank_transfer`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | None |
| [`no3ds_fail_payment`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | None |
| [`no3ds_manual_capture_credit_card`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `FAIL` | None |
| [`no3ds_manual_capture_debit_card`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `FAIL` | None |
| [`threeds_manual_capture_credit_card`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`no3ds_auto_capture_ach_bank_transfer`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_affirm`](./authorize/no3ds-auto-capture-affirm.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_afterpay_clearpay`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_alipay`](./authorize/no3ds-auto-capture-alipay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_bacs_bank_transfer`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_bancontact`](./authorize/no3ds-auto-capture-bancontact.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_credit_card`](./authorize/no3ds-auto-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_debit_card`](./authorize/no3ds-auto-capture-debit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_eps`](./authorize/no3ds-auto-capture-eps.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_giropay`](./authorize/no3ds-auto-capture-giropay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_google_pay_encrypted`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — GPAY_HOSTED_URL not set
- [`no3ds_auto_capture_ideal`](./authorize/no3ds-auto-capture-ideal.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_klarna`](./authorize/no3ds-auto-capture-klarna.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_przelewy24`](./authorize/no3ds-auto-capture-przelewy24.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_sepa_bank_transfer`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_fail_payment`](./authorize/no3ds-fail-payment.md) — assertion failed for field 'error.connector_details.message': expected 'Payment method not supported by PayU. Only UPI payments are supported is not supported by PayU' to contain 'decline'
- [`no3ds_manual_capture_credit_card`](./authorize/no3ds-manual-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_manual_capture_debit_card`](./authorize/no3ds-manual-capture-debit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`threeds_manual_capture_credit_card`](./authorize/threeds-manual-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist
