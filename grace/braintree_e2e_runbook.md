# Braintree HS → UCS → connector E2E runbook

Two checkouts:
 - UCS  : /home/infamous/hyperswitch-prism1  (branch `feat/braintree_grace`)
 - HS   : /home/infamous/hyperswitch1        (branch `main` as of a2978004a4)

## 1. UCS side

Credentials already resolve from `creds.json` → key `braintree`:
```
auth_type  SignatureKey
api_key    69fk64c44427xp8b   (public key)
key1       zzkzrfr733v3cg7k   (merchant id)
api_secret 8870337b98ca3d83d0fb8f56e4578bd8  (private key)
metadata   merchant_account_id=juspay, merchant_config_currency=USD
```
Base URLs per environment live in `config/development.toml`, `config/sandbox.toml`,
`config/production.toml` under the `braintree` key.

Start the UCS gRPC server on `localhost:8000` (that is the port HS `development.toml`
points `grpc_client.unified_connector_service.base_url` at).

## 2. HS side — routing Braintree through UCS

`should_call_unified_connector_service`
(`crates/router/src/core/unified_connector_service.rs:916`) decides Direct vs UCS. Two
levers, in this order:

**a. `ucs_only_connectors`** — `config/development.toml:1598`. Braintree is **not** in the
list today:
```
ucs_only_connectors = "imerchantsolutions, paytm, phonepe, hyperpg, revolv3,
fiservcommercehub, absa_sanlam, interpayments, payconex, dlocal, barclaycard,
tsys_transit, jpmorgan, datatrans, givepayments, tesouro, citigate, ilixium, moneris,
elavon, worldpayraft"
```
Adding `braintree` forces every Braintree call down the UCS path. This is the change that
needs a dedicated **HS PR** if Braintree is to ship as UCS-only.

**b. Rollout percentage config** — for a local/partial rollout without touching the
connector list. `build_rollout_keys_by_precedence` (`:1270`) tries these config keys,
highest precedence last-listed-first:
```
ucs_rollout_config_{org_id}
ucs_rollout_config_{org_id}_{merchant_id}
ucs_rollout_config_{org_id}_{scope}
ucs_rollout_config_{scope}
```
where `scope` = `{merchant_id}_{connector}_{flow}` for `Execute`/`RSync`, and
`{merchant_id}_{connector}_{flow}_{payment_method}` otherwise (plus
`_{payment_method_type}` for Wallet / BankRedirect / Voucher / PayLater) —
`build_merchant_rollout_scope` (`:1224`). Prefix constant:
`crates/router/src/consts.rs:359`. Set the value to `100` in the HS `config` table to send
all traffic for that scope to UCS.

Webhooks have their own gate: `should_call_unified_connector_service_for_webhooks`
(`:1385`), used from `crates/router/src/core/webhooks/incoming.rs`.

## 3. Flows to prove end to end

| Requirement | UCS flow marker | HS trigger |
|---|---|---|
| CIT, auto capture | `Authorize` | POST /payments, `capture_method=automatic` |
| Manual capture | `Authorize` + `Capture` | `capture_method=manual`, then POST /payments/{id}/capture |
| Partial capture | `Capture` | capture with `amount_to_capture` < authorized |
| $0 auth | `SetupMandate` | POST /payments, `amount=0`, `setup_future_usage=off_session` |
| MIT | `RepeatPayment` | POST /payments with `mandate_id` / `recurring_details` |
| NTID for MIT | `RepeatPayment` | MIT carrying the stored network transaction id |
| 3DS | `PreAuthenticate` / `Authenticate` / `PostAuthenticate` | `authentication_type=three_ds` |
| External 3DS | `Authorize` | external MPI `eci`/`cavv`/`ds_trans_id` pass-through |
| Refund / partial refund | `Refund`, `RSync` | POST /refunds |
| Void | `Void` | POST /payments/{id}/cancel |
| Payment + refund webhooks | `IncomingWebhook` | POST /webhooks/{merchant_id}/braintree |
| Chargeback notification | `IncomingWebhook` (dispute events) | same endpoint |

## 4. L2/L3

HS already carries `[l2_l3_data_config] enabled = "true"` (`config/development.toml:1591`),
so the L2/L3 payload reaches the connector call; the UCS Braintree transformer must map it
onto the Braintree GraphQL transaction input.
