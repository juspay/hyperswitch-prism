# Connector `paypal` / Suite `authorize`

- Service: `PaymentService/Authorize`
- Pass Rate: `36.8%` (`7` / `19`)

[Back to Overview](../../test_overview.md)

## Scenario Matrix

| Scenario | PM | PMT | Result | Prerequisites |
|:---------|:--:|:---:|:------:|:--------------|
| [`ACH Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) | ach_bank_transfer | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Affirm \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) | affirm | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Afterpay/Clearpay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) | afterpay_clearpay | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Alipay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) | ali_pay_redirect | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`BACS Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) | bacs_bank_transfer | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Bancontact \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) | bancontact_card | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Credit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-credit-card.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`Debit Card \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-debit-card.md) | card | debit | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`EPS \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-eps.md) | eps | - | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`Giropay \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) | giropay | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Google Pay (Encrypted Token) \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) | google_pay | CARD | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`iDEAL \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-ideal.md) | ideal | - | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`Klarna \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) | klarna | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Przelewy24 \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) | przelewy24 | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`SEPA Bank Transfer \| No 3DS \| Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) | sepa_bank_transfer | - | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Payment Failure \| No 3DS`](./authorize/no3ds-fail-payment.md) | card | credit | `FAIL` | `create_access_token(create_access_token)` (PASS) |
| [`Credit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`Debit Card \| No 3DS \| Manual Capture`](./authorize/no3ds-manual-capture-debit-card.md) | card | debit | `PASS` | `create_access_token(create_access_token)` (PASS) |
| [`Credit Card \| 3DS \| Manual Capture`](./authorize/threeds-manual-capture-credit-card.md) | card | credit | `PASS` | `create_access_token(create_access_token)` (PASS) |

## Failed Scenarios

- [`ACH Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-ach-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Affirm | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-affirm.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Afterpay/Clearpay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-afterpay-clearpay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Alipay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-alipay.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`BACS Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bacs-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Bancontact | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-bancontact.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Giropay | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-giropay.md) — assertion failed for field 'error': expected field to be absent or null, got {"issuerDetails":{"networkDetails":{}},"connectorDetails":{"code":"PERMISSION_DENIED","message":"PERMISSION_DENIED","reason":"description - You do not have permission to access or perform operations on this resource.;"}}
- [`Google Pay (Encrypted Token) | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-google-pay-encrypted.md) — grpcurl execution failed: browser automation engine directory does not exist: '/Users/amitsingh.tanwar/Documents/connector-service/connector-service/crates/internal/integration-tests/../../browser-automation-engine'
- [`Klarna | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-klarna.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Przelewy24 | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-przelewy24.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`SEPA Bank Transfer | No 3DS | Automatic Capture`](./authorize/no3ds-auto-capture-sepa-bank-transfer.md) — assertion failed for field 'connector_transaction_id': expected field to exist
- [`Payment Failure | No 3DS`](./authorize/no3ds-fail-payment.md) — assertion failed for field 'error': expected field to exist