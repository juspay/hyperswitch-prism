# Connector `nexixpay` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `0.0%` (`0` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `FAIL` | `create_customer(create_customer)` (FAIL) |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `FAIL` | None |

## Failed Scenarios

- [`ACH Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_ach_bank_transfer': Payment method BankTransfer(AchBankTransfer) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Affirm | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_affirm': Payment method PayLater(AffirmRedirect) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Afterpay/Clearpay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_afterpay_clearpay': Payment method PayLater(AfterpayClearpayRedirect) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Alipay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_alipay': Payment method Wallet(AliPayRedirect(AliPayRedirection)) is not supported by Nexixpay (code: BAD_REQUEST)
- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_bacs_bank_transfer': Payment method BankTransfer(BacsBankTransfer) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Bancontact | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_bancontact': Payment method BankRedirect(BancontactCard { card_number: Some(CardNumber(411111**********)), card_exp_month: Some(*** alloc::string::String ***), card_exp_year: Some(*** alloc::string::String ***), card_holder_name: Some(*** alloc::string::String ***) }) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Credit Card | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_credit_card': Missing required field: authentication_data (must be present for 3DS flow) (code: BAD_REQUEST)
- [`Debit Card | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_debit_card': Missing required field: authentication_data (must be present for 3DS flow) (code: BAD_REQUEST)
- [`EPS | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_eps': Payment method BankRedirect(Eps { bank_name: None, country: None }) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Giropay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_giropay': Payment method BankRedirect(Giropay { bank_account_bic: None, bank_account_iban: None, country: None }) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — grpcurl execution failed: [google_pay_token_gen] nexixpay/authorize/no3ds_auto_capture_google_pay_encrypted: npm exited with exit status: 1
- [`iDEAL | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_ideal': Payment method BankRedirect(Ideal { bank_name: None }) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Klarna | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_klarna': Payment method PayLater(KlarnaRedirect) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Przelewy24 | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_przelewy24': Payment method BankRedirect(Przelewy24 { bank_name: None }) is not supported by Nexixpay (code: BAD_REQUEST)
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_auto_capture_sepa_bank_transfer': Payment method BankTransfer(SepaBankTransfer) is not supported by Nexixpay (code: BAD_REQUEST)
- [`Payment Failure | No 3DS`](./authorize/no3ds-fail-payment.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_fail_payment': Missing required field: authentication_data (must be present for 3DS flow) (code: BAD_REQUEST)
- [`Credit Card | No 3DS | Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_manual_capture_credit_card': Missing required field: authentication_data (must be present for 3DS flow) (code: BAD_REQUEST)
- [`Debit Card | No 3DS | Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) — sdk call failed: sdk request transformer failed for 'authorize/no3ds_manual_capture_debit_card': Missing required field: authentication_data (must be present for 3DS flow) (code: BAD_REQUEST)
- [`Credit Card | 3DS | Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) — assertion failed for field 'connector_transaction_id': expected field to exist