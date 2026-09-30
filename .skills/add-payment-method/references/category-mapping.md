# Payment Method Category Mapping

This reference maps common payment method names to their `PaymentMethodData` enum variant,
inner data type, and Rust enum path. Use it to determine which category a requested payment
method belongs to before implementing it.

## Mapping Table

| Payment Method Name | Category | PaymentMethodData Variant | Inner Enum Variant |
|---------------------|----------|---------------------------|--------------------|
| Credit Card / Debit Card | Card | `PaymentMethodData::Card(card)` | N/A (struct, not enum) |
| Apple Pay | Wallet | `PaymentMethodData::Wallet(w)` | `WalletData::ApplePay(ApplePayWalletData)` -- the decrypted/encrypted split is inside `ApplePayWalletData.payment_data: ApplePayPaymentData`, not on the router data |
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
| Boleto | Voucher | `PaymentMethodData::Voucher(v)` | `VoucherData::Boleto(Box<BoletoVoucherData>)` |
| OXXO | Voucher | `PaymentMethodData::Voucher(v)` | `VoucherData::Oxxo` (unit variant) |
| Alfamart / Indomaret | Voucher | `PaymentMethodData::Voucher(v)` | `VoucherData::Alfamart(Box<AlfamartVoucherData>)`, `VoucherData::Indomaret(Box<IndomaretVoucherData>)` |
| 7-Eleven / Lawson / MiniStop / FamilyMart / Seicomart / PayEasy (konbini) | Voucher | `PaymentMethodData::Voucher(v)` | `VoucherData::SevenEleven(Box<JCSVoucherData>)`, `VoucherData::Lawson(..)`, `VoucherData::MiniStop(..)`, `VoucherData::FamilyMart(..)`, `VoucherData::Seicomart(..)`, `VoucherData::PayEasy(..)` -- all six wrap the same `JCSVoucherData` |
| Efecty / PagoEfectivo / RedCompra / RedPagos | Voucher | `PaymentMethodData::Voucher(v)` | `VoucherData::Efecty`, `VoucherData::PagoEfectivo`, `VoucherData::RedCompra`, `VoucherData::RedPagos` -- all unit variants |
| Card without CVC | Card (no CVC) | `PaymentMethodData::CardWithNoCvc(card)` | N/A (struct `CardWithNoCvc`, 11 fields, **no `card_cvc`**) |
| Stored card + network transaction ID (MIT) | Card (stored NTID) | `PaymentMethodData::CardDetailsForNetworkTransactionId(c)` | N/A (struct `CardDetailsForNetworkTransactionId`, 10 fields, no CVC) |
| Decrypted wallet token + network transaction ID (MIT) | Wallet token (stored NTID) | `PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(d)` | N/A (struct: `decrypted_token`, `token_exp_month`, `token_exp_year`, `card_holder_name`, `eci`, `token_source: Option<TokenSource>` where `TokenSource` is `GooglePay` / `ApplePay`) |
| KNET | CardRedirect | `PaymentMethodData::CardRedirect(cr)` | `CardRedirectData::Knet {}` |
| Benefit | CardRedirect | `PaymentMethodData::CardRedirect(cr)` | `CardRedirectData::Benefit {}` |
| MoMo ATM | CardRedirect | `PaymentMethodData::CardRedirect(cr)` | `CardRedirectData::MomoAtm {}` |
| Generic card redirect | CardRedirect | `PaymentMethodData::CardRedirect(cr)` | `CardRedirectData::CardRedirect {}` |
| Mandate-only / stored-mandate charge | MandatePayment | `PaymentMethodData::MandatePayment` | N/A (unit variant, no inner data -- the mandate ID travels on the request, not here) |
| DuitNow | RealTimePayment | `PaymentMethodData::RealTimePayment(rtp)` (Box) | `RealTimePaymentData::DuitNow {}` |
| FPS (Hong Kong) | RealTimePayment | `PaymentMethodData::RealTimePayment(rtp)` (Box) | `RealTimePaymentData::Fps {}` |
| PromptPay | RealTimePayment | `PaymentMethodData::RealTimePayment(rtp)` (Box) | `RealTimePaymentData::PromptPay {}` |
| VietQR | RealTimePayment | `PaymentMethodData::RealTimePayment(rtp)` (Box) | `RealTimePaymentData::VietQr {}` |
| Connector-side payment token (Apple Pay / Google Pay token handoff) | PaymentMethodToken | `PaymentMethodData::PaymentMethodToken(t)` | N/A (struct: `token: Secret<String>`, `token_payment_method_type: Option<TokenPaymentMethod>` where `TokenPaymentMethod` is `ApplePay` / `GooglePay`) |
| Open Banking PIS | OpenBanking | `PaymentMethodData::OpenBanking(ob)` | `OpenBankingData::OpenBankingPIS {}` (the only variant) |
| Network token (token PAN + cryptogram) | NetworkToken | `PaymentMethodData::NetworkToken(nt)` | N/A (struct `NetworkTokenData`: `token_number: cards::NetworkToken`, `token_exp_month`, `token_exp_year`, `token_cryptogram`, `eci`, plus card metadata) |

## Category → Pattern File

Once you have the category, the pattern file is
`references/payment-method-patterns/<category>.md` (kebab-case), and the full 21-row
category → file table lives in `SKILL.md`. Several of those files are symlinks into the
wider rulesbook corpus at `grace/rulesbook/codegen/guides/patterns/authorize/<category>/`
(snake_case dir, `pattern_authorize_<category>.md`) -- that corpus is the authoritative,
longer-form set. Read it whenever the skill-local file is too thin.

Worked examples:
- "add Pix to X" → Pix is BankTransfer → `references/payment-method-patterns/bank-transfer.md`
- "add Boleto to X" → Boleto is Voucher → `references/payment-method-patterns/voucher.md`
  (symlink → `grace/rulesbook/codegen/guides/patterns/authorize/voucher/pattern_authorize_voucher.md`)

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
   directly in `crates/types-traits/domain_types/src/payment_method_data.rs`
   (`pub enum PaymentMethodData<T: PaymentMethodDataTypes>`) to find the correct variant.
   At HEAD it has 21 variants and this table now has at least one row for every one of them,
   so a miss means either a new variant landed or the name maps to a sub-variant of an
   existing category. Two shapes are routinely mis-modelled:
   `PaymentMethodData::PaymentMethodToken` wraps a **struct** (`token`,
   `token_payment_method_type`), not an enum of decrypted wallet payloads; and
   `PaymentMethodData::MandatePayment` and `PaymentMethodData::Reward` are **unit** variants
   with no inner data at all.

## Special Cases

| Scenario | Correct Category | Common Mistake |
|----------|-----------------|----------------|
| Apple Pay | Wallet (`WalletData::ApplePay`) | Putting it under MobilePayment |
| Samsung Pay | Wallet (`WalletData::SamsungPay`) | Putting it under MobilePayment |
| Paze | Wallet (`WalletData::Paze`) | Not recognizing it as a wallet |
| Pix | BankTransfer (`BankTransferData::Pix`) | Putting it under BankRedirect |
| Bancontact | BankRedirect (`BankRedirectData::BancontactCard`) | Putting it under Card (it has card fields but is a redirect) |
| Boleto | Voucher (`VoucherData::Boleto(Box<BoletoVoucherData>)`) | Putting it under BankTransfer |
| OXXO | Voucher (`VoucherData::Oxxo`, a unit variant) | Putting it under BankTransfer |
| Reward / Loyalty | Reward (`PaymentMethodData::Reward`) | Creating a new variant (Reward is a unit variant, no inner data) |
| KNET / Benefit / MoMo ATM | CardRedirect (`CardRedirectData::Knet {}` etc.) | Putting them under Card because the name says "card" |
| PromptPay / DuitNow / FPS / VietQR | RealTimePayment (Box-wrapped) | Putting them under BankTransfer or Wallet |
| Card details with no CVC | `PaymentMethodData::CardWithNoCvc` | Routing to `PaymentMethodData::Card` and synthesising a CVC -- never fabricate one |
| MIT re-use of a stored card | `PaymentMethodData::CardDetailsForNetworkTransactionId` | Confusing it with `CardWithNoCvc`; the NTID variant is keyed on a stored network transaction ID |
| Apple Pay / Google Pay token handed straight to the connector | `PaymentMethodData::PaymentMethodToken` (struct) | Treating it as a `WalletData` sub-variant |
