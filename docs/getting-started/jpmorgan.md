# JPMorgan payment inputs

Use UCS for JPMorgan payment processing.
The Hyperswitch connector only supplies routing capabilities for these flows.

## Decrypted wallets

Supply the decrypted credential, credential expiry, and wallet network.
Do not use the payment account reference as a payment credential.

| Input | JPMorgan credential type | Required proof for a cardholder payment |
|---|---|---|
| Apple Pay without a merchant token identifier | `DEVICE_TOKEN` | Wallet cryptogram |
| Apple Pay with a merchant token identifier | `NETWORK_TOKEN` | Wallet cryptogram |
| Google Pay `CRYPTOGRAM_3DS` | `DEVICE_TOKEN` | Wallet cryptogram |
| Google Pay `PAN_ONLY` | `PAN` | No token cryptogram |

Set `GooglePayDecryptedData.auth_method` to `PAN_ONLY` or `CRYPTOGRAM_3DS`.
Missing cryptogram data does not establish `PAN_ONLY`.
Supply an explicit ECI when your wallet supplies one.
UCS preserves that value.
For Hyperswitch predecrypted input, set `apple_pay_combined.support_predecrypted_token` or `google_pay.support_predecrypted_token` to `true` in the merchant connector account metadata.
Supply the predecrypted token; these flags do not provision wallet certificates or create wallet proof.

## Recurring payments

For a scheduled initial cardholder payment, set `mit_category` to `RECURRING_MIT`.
Provide connector context in the JSON value of `metadata`:

```json
{
  "jpmorgan": {
    "agreementId": "your-original-agreement",
    "isVariableAmount": false
  }
}
```

Mastercard scheduled payments require `isVariableAmount`.
Cartes Bancaires scheduled payments require a positive `recurringNumber`.
Keep the returned original network transaction ID, transaction link ID, and reusable `mandate_metadata`.
Keep the original agreement across subsequent scheduled payments.

Provide `merchant_order_id` when you need a merchant order reference for an enhanced payment or MIT.
UCS maps it to `merchantOrderNumber` without changing it.
Use 1–40 ASCII letters, digits, spaces, periods, or hyphens.
This optional field does not change ordinary-card request serialization.

Call `RecurringPaymentService/Charge` with the supplied credential and `connector_recurring_payment_id.network_mandate_id`.
Supply `network_transaction_id` from the original cardholder transaction.
Supply `transaction_link_id` from that transaction when available; Mastercard requires it.
Do not replace either original identifier with an identifier from a later merchant-initiated transaction.
Do not replay cardholder cryptograms or 3DS proof on a merchant-initiated transaction.

For `decrypted_wallet_token_details_for_network_transaction_id`, retain the wallet source and provide the original classification in connector context:

```json
{
  "jpmorgan": {
    "accountNumberType": "DEVICE_TOKEN",
    "walletProvider": "GOOGLE_PAY"
  }
}
```

Merge this classification with the original agreement and identifiers rather than replacing that context.
Automatic saved-wallet retrieval through Hyperswitch `payment_method_id` is not implemented by this change.
Supply the credential and original context explicitly.

## Zero-dollar setup

Call `PaymentService/SetupRecurring` with a zero or absent amount.
UCS sends `POST /verifications` without an amount or capture method.
A successful response uses the terminal setup status `CHARGED`; it does not represent a funds charge or a capturable authorization.
Do not use the returned verification resource ID as a connector payment token.
Use the original network identifiers for subsequent supplied-credential payments.

## 3DS

For pass-through authentication, supply CAVV, ECI, protocol version, and the provider-required authentication transaction identifier.
Use the existing `AuthenticationData.ds_transaction_id` carrier unchanged.
For Visa, supply the XID required by JPMorgan in that carrier.
For Mastercard, supply the directory server transaction ID.
Do not synthesize or substitute a different transaction identifier.
Cartes Bancaires 3DS is refused because its additional evidence is not mapped.

For native 3DS, provide an HTTPS `complete_authorize_url`, browser information, and the required account-holder contact and billing fields.
Launch the returned orchestration redirect with GET and preserve its query parameters.
Retain the returned attempt-level `connector_feature_data` for continuation.
Return it unchanged so continuation preserves the original resource and capture intent.
Do not put 3DS operation state in caller `metadata`.
Continuation retrieves the original payment or verification through an authenticated GET.
Callback parameters do not establish payment success.
Do not issue another authorization POST to complete authentication.

## Validation limits

JPMorgan's hosted mock is not evidence of funds movement or successful browser authentication.
Validate genuine wallet proof, merchant entitlements, and browser completion against your provisioned environment before enabling these flows.
