# Payment Method Category Mapping

This reference maps common payment method names to their `PaymentMethodData` enum variant,
inner data type, and Rust enum path. Use it to determine which category a requested payment
method belongs to before implementing it.

## Mapping Table

| Payment Method Name | Category | PaymentMethodData Variant | Inner Enum Variant |
|---------------------|----------|---------------------------|--------------------|
| Credit Card / Debit Card | Card | `PaymentMethodData::Card(card)` | N/A (struct, not enum) |
| Apple Pay | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::ApplePay(ApplePayWalletData)` |
| Apple Pay (redirect) | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::ApplePayRedirect(Box<ApplePayRedirectData>)` |
| Apple Pay (3rd party SDK) | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::ApplePayThirdPartySdk(Box<ApplePayThirdPartySdkData>)` |
| Google Pay | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::GooglePay(GooglePayWalletData)` |
| Google Pay (redirect) | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::GooglePayRedirect(Box<GooglePayRedirectData>)` |
| PayPal (redirect) | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::PaypalRedirect(PaypalRedirection)` |
| PayPal (SDK) | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::PaypalSdk(PayPalWalletData)` |
| AliPay | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::AliPayRedirect(AliPayRedirection)` |
| AliPay QR | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::AliPayQr(Box<AliPayQr>)` |
| WeChat Pay | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::WeChatPayRedirect(Box<WeChatPayRedirection>)` |
| Samsung Pay | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::SamsungPay(Box<SamsungPayWalletData>)` |
| Paze | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::Paze(Box<PazeWalletData>)` |
| ACH Bank Transfer | BankTransfer | `PaymentMethodData::BankTransfer(bt)` | `BankTransferData::AchBankTransfer {}` |
| SEPA Bank Transfer | BankTransfer | `PaymentMethodData::BankTransfer(bt)` | `BankTransferData::SepaBankTransfer {}` |
| BACS Bank Transfer | BankTransfer | `PaymentMethodData::BankTransfer(bt)` | `BankTransferData::BacsBankTransfer {}` |
| Pix | BankTransfer | `PaymentMethodData::BankTransfer(bt)` | `BankTransferData::Pix { pix_key, cpf, cnpj, .. }` |
| Multibanco | BankTransfer | `PaymentMethodData::BankTransfer(bt)` | `BankTransferData::MultibancoBankTransfer {}` |
| ACH Direct Debit | BankDebit | `PaymentMethodData::BankDebit(bd)` | `BankDebitData::AchBankDebit { account_number, routing_number, .. }` |
| SEPA Direct Debit | BankDebit | `PaymentMethodData::BankDebit(bd)` | `BankDebitData::SepaBankDebit { iban, .. }` |
| BACS Direct Debit | BankDebit | `PaymentMethodData::BankDebit(bd)` | `BankDebitData::BacsBankDebit { account_number, sort_code, .. }` |
| BECS Direct Debit | BankDebit | `PaymentMethodData::BankDebit(bd)` | `BankDebitData::BecsBankDebit { account_number, bsb_number, .. }` |
| iDEAL | BankRedirect | `PaymentMethodData::BankRedirect(br)` | `BankRedirectData::Ideal { bank_name }` |
| Sofort | BankRedirect | `PaymentMethodData::BankRedirect(br)` | `BankRedirectData::Sofort { .. }` |
| Giropay | BankRedirect | `PaymentMethodData::BankRedirect(br)` | `BankRedirectData::Giropay { .. }` |
| EPS | BankRedirect | `PaymentMethodData::BankRedirect(br)` | `BankRedirectData::Eps { bank_name, country }` |
| Bancontact | BankRedirect | `PaymentMethodData::BankRedirect(br)` | `BankRedirectData::BancontactCard { .. }` |
| Przelewy24 | BankRedirect | `PaymentMethodData::BankRedirect(br)` | `BankRedirectData::Przelewy24 { bank_name }` |
| UPI Collect | UPI | `PaymentMethodData::Upi(upi)` | `UpiData::UpiCollect(UpiCollectData)` |
| UPI Intent | UPI | `PaymentMethodData::Upi(upi)` | `UpiData::UpiIntent(UpiIntentData)` |
| UPI QR | UPI | `PaymentMethodData::Upi(upi)` | `UpiData::UpiQr(UpiQrData)` |
| Klarna | BNPL | `PaymentMethodData::PayLater(pl)` | `PayLaterData::KlarnaRedirect {}` |
| Afterpay / Clearpay | BNPL | `PaymentMethodData::PayLater(pl)` | `PayLaterData::AfterpayClearpayRedirect {}` |
| Affirm | BNPL | `PaymentMethodData::PayLater(pl)` | `PayLaterData::AffirmRedirect {}` |
| Atome | BNPL | `PaymentMethodData::PayLater(pl)` | `PayLaterData::AtomeRedirect {}` |
| Cryptocurrency | Crypto | `PaymentMethodData::Crypto(crypto)` | N/A (struct: `CryptoData { pay_currency, network }`) |
| Givex Gift Card | GiftCard | `PaymentMethodData::GiftCard(gc)` | `GiftCardData::Givex(GiftCardDetails)` |
| PaySafeCard | GiftCard | `PaymentMethodData::GiftCard(gc)` | `GiftCardData::PaySafeCard {}` |
| Carrier Billing | MobilePayment | `PaymentMethodData::MobilePayment(mp)` | `MobilePaymentData::DirectCarrierBilling { msisdn, client_uid }` |
| Loyalty / Reward | Reward | `PaymentMethodData::Reward` | N/A (unit variant, no inner data) |

## How to Determine Category from a Payment Method Name

1. **Check the table above first.** Most common payment methods are listed.

2. **Apply these rules for ambiguous names:**
   - "Apple Pay" is always **Wallet**, never MobilePayment. The `MobilePayment` category
     is exclusively for carrier/direct-carrier-billing scenarios.
   - "PayPal" is always **Wallet** (either `PaypalRedirect` or `PaypalSdk`).
   - "SEPA" alone is ambiguous: it could be **BankTransfer** (`SepaBankTransfer`) or
     **BankDebit** (`SepaBankDebit`). Check the connector's API docs to determine which.
     If the funds are pulled (direct debit), use BankDebit. If pushed (credit transfer),
     use BankTransfer.
   - "ACH" is similarly ambiguous between BankTransfer and BankDebit. Apply the same
     pull vs. push logic.
   - "Pix" is **BankTransfer**, not BankRedirect, even though it involves a QR code.

3. **If the payment method is not in the table**, check the `PaymentMethodData` enum
   directly in `crates/types-traits/domain_types/src/payment_method_data.rs` to find
   the correct variant.

## Special Cases

| Scenario | Correct Category | Common Mistake |
|----------|-----------------|----------------|
| Apple Pay | Wallet (`WalletData::ApplePay`) | Putting it under MobilePayment |
| Samsung Pay | Wallet (`WalletData::SamsungPay`) | Putting it under MobilePayment |
| Paze | Wallet (`WalletData::Paze`) | Not recognizing it as a wallet |
| Pix | BankTransfer (`BankTransferData::Pix`) | Putting it under BankRedirect |
| Bancontact | BankRedirect (`BankRedirectData::BancontactCard`) | Putting it under Card (it has card fields but is a redirect) |
| Boleto | Voucher (`VoucherData::Boleto`) | Putting it under BankTransfer |
| OXXO | Voucher (`VoucherData::Oxxo`) | Putting it under BankTransfer |
| Reward / Loyalty | Reward (`PaymentMethodData::Reward`) | Creating a new variant (Reward is a unit variant, no inner data) |
