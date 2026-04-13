# Payment Methods

Prism's `authorize` call accepts a `payment_method` that describes how the payer's instrument is represented. Set exactly one variant inside that object — Prism handles the connector-specific transformation.

| Payment method | Example fields |
|---|---|
| [Connector Token](./connector-token.md) | `connector_token` / `payment_method.token` |
| Card | `payment_method.card` |
| Wallets | `payment_method.apple_pay` / `google_pay` / `paypal_redirect` / `paypal_sdk` / ... |
| Bank Transfer | `payment_method.ach_bank_transfer` / `sepa_bank_transfer` / `bacs_bank_transfer` / ... |
| Direct Debit | `payment_method.ach` / `sepa` / `bacs` / ... |
| Online Banking | `payment_method.ideal` / `sofort` / `giropay` / `eps` / ... |
| BNPL | `payment_method.klarna` / `afterpay_clearpay` / `affirm` |
| UPI | `payment_method.upi_collect` / `upi_intent` / `upi_qr` |
| Voucher / Cash | `payment_method.alfamart` / `indomaret` / `oxxo` / ... |
| Crypto | `payment_method.crypto` |
| Gift Card | `payment_method.givex` / ... |

> **Non-PCI flow:** If card details are collected via the processor's JS SDK (Stripe.js, Adyen Drop-in, etc.), call `payments.tokenAuthorize()` with a top-level `connector_token` field instead of `payments.authorize()` — the `payment_method` field is absent in that call. See [Connector Token](./connector-token.md).

