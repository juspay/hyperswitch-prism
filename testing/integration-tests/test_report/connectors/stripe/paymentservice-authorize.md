# Connector `stripe` / Suite `PaymentService/Authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `89.5%` (`17` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Affirm \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-affirm.md) | affirm | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Alipay \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Bancontact \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Credit Card \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Debit Card \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`EPS \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-eps.md) | eps | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Giropay \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-giropay.md) | giropay | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `FAIL` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`iDEAL \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-ideal.md) | ideal | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Klarna \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-klarna.md) | klarna | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Payment Failure \| No 3DS`](./paymentservice-authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Credit Card \| No 3DS \| Manual Capture`](./paymentservice-authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Debit Card \| No 3DS \| Manual Capture`](./paymentservice-authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |
| [`Credit Card \| 3DS \| Manual Capture`](./paymentservice-authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | `CustomerService/Create(create_customer)` (PASS) -> `PaymentMethodService/Tokenize(tokenize_credit_card)` (PASS) |

## Failed Scenarios

- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./paymentservice-authorize/no3ds-auto-capture-google-pay-encrypted.md) — GPAY_HOSTED_URL not set
- [`Payment Failure | No 3DS`](./paymentservice-authorize/no3ds-fail-payment.md) — assertion failed for field 'error': expected field to exist
