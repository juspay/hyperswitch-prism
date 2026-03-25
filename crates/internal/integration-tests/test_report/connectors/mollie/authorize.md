# Connector `mollie` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `5.3%` (`1` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`no3ds_auto_capture_ach_bank_transfer`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_affirm`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_afterpay_clearpay`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_alipay`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_bacs_bank_transfer`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_bancontact`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_credit_card`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_debit_card`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_eps`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_giropay`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_google_pay_encrypted`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `SKIP` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_ideal`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_klarna`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_przelewy24`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_auto_capture_sepa_bank_transfer`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_fail_payment`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_manual_capture_credit_card`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`no3ds_manual_capture_debit_card`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`threeds_manual_capture_credit_card`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_customer(create_customer)` (FAIL) |

## Failed Scenarios

- [`no3ds_auto_capture_ach_bank_transfer`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_affirm`](./authorize/no3ds-auto-capture-affirm.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_afterpay_clearpay`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_alipay`](./authorize/no3ds-auto-capture-alipay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_bacs_bank_transfer`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_bancontact`](./authorize/no3ds-auto-capture-bancontact.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_credit_card`](./authorize/no3ds-auto-capture-credit-card.md) — assertion failed for field 'status': expected one of ["CHARGED", "AUTHORIZED"], got "AUTHENTICATION_PENDING"
- [`no3ds_auto_capture_debit_card`](./authorize/no3ds-auto-capture-debit-card.md) — assertion failed for field 'status': expected one of ["CHARGED", "AUTHORIZED"], got "AUTHENTICATION_PENDING"
- [`no3ds_auto_capture_eps`](./authorize/no3ds-auto-capture-eps.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_giropay`](./authorize/no3ds-auto-capture-giropay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_google_pay_encrypted`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — GPAY_HOSTED_URL not set
- [`no3ds_auto_capture_ideal`](./authorize/no3ds-auto-capture-ideal.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_klarna`](./authorize/no3ds-auto-capture-klarna.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_przelewy24`](./authorize/no3ds-auto-capture-przelewy24.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_auto_capture_sepa_bank_transfer`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`no3ds_fail_payment`](./authorize/no3ds-fail-payment.md) — assertion failed for field 'error': expected field to exist
- [`no3ds_manual_capture_credit_card`](./authorize/no3ds-manual-capture-credit-card.md) — assertion failed for field 'status': expected one of ["AUTHORIZED"], got "AUTHENTICATION_PENDING"
- [`no3ds_manual_capture_debit_card`](./authorize/no3ds-manual-capture-debit-card.md) — assertion failed for field 'status': expected one of ["AUTHORIZED"], got "AUTHENTICATION_PENDING"
