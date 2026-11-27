# Paynamics Landbank

This connector supports Landbank online banking (`br_landbank_ph`) only. UCS Authorize creates an automatic sale; the customer completes payment on the returned redirect page, and payment Sync retrieves the final status. Manual authorization, Capture, Void, reversal, refunds, refund sync, webhooks, cards, wallets and other bank channels are not implemented in this change.

## Configuration

Set `x-connector: paynamics` and provide typed `PaynamicsConfig` through `x-connector-config` with merchant ID, merchant key, Basic username and password. Credential values must come from your merchant account. The sandbox base URL is `https://payin.payserv.net/paygate/`. Production has no default URL; obtain the production REST URL from Paynamics and configure it explicitly.

Legacy `MultiAuthKey` configuration maps `api_key` to merchant ID, `key1` to merchant key, `api_secret` to username and `key2` to password.

## Authorize

Use `/composite/payments/authorize` (or `types.PaymentService/Authorize`) with PHP money, `AUTOMATIC` capture, a unique merchant transaction/request reference, customer and billing information, order details, a valid return URL and a notification URL. The notification URL is required by Paynamics even though UCS webhook processing is outside this PR.

The HTTP payment method is:

```json
{
  "payment_method": {
    "payment_method": {
      "local_bank_transfer": {
        "bank_code": "br_landbank_ph"
      }
    }
  }
}
```

Paynamics receives `onlinebanktransfer / br_landbank_ph / url_link`. Follow `redirection_data` to complete payment. `GR033` means pending; it is not proof of payment.

## Sync

Use `/composite/payments/get` (or `types.PaymentService/Get`) with the payment provider ID, the unchanged `connector_feature_data` from Authorize and a fresh `merchant_request_id` for each new poll. The carrier retains the original sale request ID; this is sent as `org_trxid2` to `transactions/query`. Do not substitute the provider response ID. `GR001` maps to `CHARGED`; query errors such as `GR122` and duplicate-request errors do not establish a terminal payment failure.

Provider sandbox queries have intermittently taken longer than 30 seconds. This PR does not change global timeout settings. A timeout alone does not establish whether a sale succeeded; query the original sale rather than creating a duplicate payment.

## Regression coverage

The connector-private `no3ds_auto_capture_landbank` scenario sends a PHP automatic sale with `local_bank_transfer.bank_code = br_landbank_ph`. The Get suite replaces its default card dependency with this scenario and forwards the sale's complete `connector_feature_data`, provider ID, and amount. The scenarios explicitly generate `merchant_request_id` in each request body, so each Sync execution uses a new poll reference rather than reusing the sale transaction ID. Card and other payment-method scenarios are filtered out for this connector.

Run the credential-free regression wiring checks with:

```sh
cargo test -p integration-tests --test paynamics_landbank
```

With separately provisioned sandbox credentials, the existing regression runner can execute the Landbank sale and dependent Sync:

```sh
cargo run -p integration-tests --bin test_ucs -- --connector paynamics --suite PaymentService/Get --scenario sync_payment --interface grpc
```

This runner creates a sandbox sale. Its status may remain `PENDING` until the customer completes the bank redirect; these scenarios do not claim automated bank-page completion or live certification.

## Evidence and limits

Merchant sandbox testing completed a Landbank redirect payment and confirmed `GR001` / `CHARGED` through Sync. CI credentials and live regression certification are not provisioned. Production binding remains to be confirmed with Paynamics. Sandbox observations do not certify production behavior.
