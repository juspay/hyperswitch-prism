# Braintree — live end-to-end verification report

Run: 2026-09-15, branch `feat/braintree_grace`, against the **live Braintree
sandbox** (`https://payments.sandbox.braintree-api.com/graphql`,
`Braintree-Version: 2019-01-01`) via a locally built `grpc-server` on port 8056.
Every result below is a real gRPC call to UCS that made a real HTTPS call to
Braintree. No mocks.

Credentials resolved from `creds.json` key `braintree` (gitignored).

## Summary

| Requirement | Flow | Result |
|---|---|---|
| CIT, auto capture | Tokenize → Authorize | **PASS** — `CHARGED` |
| Manual capture | Authorize(MANUAL) → Capture | **PASS** — `AUTHORIZED` → `CHARGED` |
| Partial capture | Capture 3000 of 6000 | **PASS** — see caveat C1 |
| Void | Authorize(MANUAL) → Void | **PASS** — `VOIDED` |
| Refund | settle → Refund | **PASS** — `REFUND_SUCCESS` |
| Partial refund | settle → Refund 3000 of 6000 | **PASS** — `REFUND_SUCCESS` |
| PSync | PaymentService/Get | **PASS** — `CHARGED` |
| RSync | RefundService/Get | **PASS after a fix** — see F1 |
| $0 auth | SetupRecurring, amount 0 | **PASS** — returns mandate reference |
| MIT | RecurringPaymentService/Charge | **PASS** — `CHARGED` |
| NTID on MIT | same | **PASS** — NTID returned, see below |
| AVS check | Authorize response | **PASS** — codes surfaced |
| CVV check | Authorize response | **PASS** — code surfaced |
| MAC / issuer codes | Authorize response | **PASS** — processor code + text surfaced |
| Payment webhook | EventService Parse+Handle | **PASS** — `PAYMENT_INTENT_SUCCESS` / `CHARGED` |
| Refund webhook | EventService Parse+Handle | **PASS** — `WEBHOOK_REFUND_FAILURE` |
| Chargeback notification | EventService Parse+Handle | **PASS** — `WEBHOOK_DISPUTE_OPENED`, `ACTIVE_DISPUTE` |
| Webhook source verification | HandleEvent | **PASS** — `sourceVerified: true`; absent on a tampered signature |
| 3DS PreAuthenticate | PaymentMethodAuthenticationService | **PASS** — `DEVICE_DATA_COLLECTION_PENDING` |
| 3DS Authenticate | same | **EXECUTES; cannot be proven** — see B1 |
| 3DS PostAuthenticate | same | **BLOCKED by B1** |
| External 3DS | Authorize | not exercised live (needs an MPI) |
| L2/L3, dynamic descriptor | Authorize | not exercised live (needs a settled-report check) |
| Smart retry / GSM | — | not code; see `braintree_hs_side_notes.md` |

## Evidence worth quoting

**Authorize (Tokenize → Authorize), `CHARGED`:**
```
connectorTransactionId: dHJhbnNhY3Rpb25fZDl5YzlucTk
status:                 CHARGED
networkTransactionId:   020260915151313
```
`connectorResponse.additionalPaymentMethodData.card.paymentChecks` base64-decodes to:
```json
{ "avs_street_address_response_code": "M", "avs_postal_code_response_code": "M",
  "cvv_response_code": "M", "avs_street_address_response": "MATCHES",
  "avs_postal_code_response": "MATCHES", "cvv_response": "MATCHES",
  "processor_response_code": "1000", "processor_response_text": "Approved" }
```
That single field is the live proof for four separate requirements — AVS check,
CVV check response, MAC/processor codes, and issuer error mapping input.

**NTID on MIT** — `RecurringPaymentService/Charge` against a vaulted mandate
returned `networkTransactionId: 020260915152236`, and the raw response shows the
non-obvious traversal the RepeatPayment run identified is the one that works:
```
"paymentMethodSnapshot": { "networkTransactionId": "020260915152236" }
```
`Transaction` has 43 fields and `networkTransactionId` is not one of them — only
`CreditCardTransactionDetails` under the `paymentMethodSnapshot` union carries it.
This was a Braintree-vaulted (Regime A) MIT, so `externalVault` was correctly
**omitted** per RULE NT-1.

**Webhooks** — payloads signed locally with Braintree's documented chain
`HMAC-SHA1(SHA1(private_key), bt_payload)`, using `<subject>` nesting:
```
dispute_opened     -> WEBHOOK_DISPUTE_OPENED   reference.dispute.connectorTransactionId=txn_e2e_001
                      HandleEvent: DISPUTE_OPENED / ACTIVE_DISPUTE, sourceVerified: true
transaction_settled-> PAYMENT_INTENT_SUCCESS   HandleEvent: CHARGED, sourceVerified: true
refund_failed      -> WEBHOOK_REFUND_FAILURE   HandleEvent: REFUND_FAILURE, sourceVerified: true
tampered signature -> body still parsed, sourceVerified ABSENT (reported, not gated)
```
These payloads use the `<subject>` level, so they are also the live proof that
the webhook run's B1 fix (missing `<subject>` nesting) is real and correct.

## Findings

### F1 — RSync was unreachable from Hyperswitch (FIXED, commit 391b517b9)

`BraintreeRSyncRequest`'s `TryFrom` required `currency` inside the **optional**
`refund_connector_metadata` blob and hard-errored when absent:
```
MISSING_REQUIRED_FIELD: currency
```
Hyperswitch does not populate that blob — it sends the currency on the
first-class `RefundServiceGetRequest.refund_amount` (proto field 15), which the
code ignored. A settled refund could never be read back. RSync searches by refund
id and sends neither amount nor currency to Braintree, so the guard could only
reject requests that would otherwise have succeeded. Now resolved in precedence
order (`refund_money` → metadata → skip), with three tests. Verified live both
with `refund_amount` alone and with no currency at all.

### B1 — 3DS cannot be proven on this sandbox account (BLOCKER, not a code defect)

`performThreeDSecureLookup` returns `authenticationStatus: LOOKUP_ERROR` for every
card tried — `4111111111111111` and Braintree's own 3DS test cards
`4000000000001000`, `4000000000001091`, `4000000000001109`.

**This is not the connector.** A raw `curl` straight to Braintree with no UCS in
the path returns the same, with and without `merchantAccountId: juspay`:
```json
{"threeDSecureLookupData":{"acsUrl":null,"pareq":null,"version":null,"transactionId":null},
 "paymentMethod":{"details":{"threeDSecure":{"authentication":{"authenticationStatus":"LOOKUP_ERROR"}}}}}
```
3D Secure is not provisioned on this sandbox gateway. The connector behaves
correctly given that response — it maps `LOOKUP_ERROR` → `AUTHENTICATION_FAILED`,
surfaces the full authentication block, and round-trips its state through
`connector_feature_data.braintree_three_ds`.

Consequence: the frictionless and challenge happy paths, and PostAuthenticate's
`node(id:)` readback, cannot be exercised here. PostAuthenticate against a
LOOKUP_ERROR payment method returns `An object with this ID was not found.`

**This partly contradicts the Authenticate run's report**, which described
exercising four 3DS test cards and observing frictionless approvals with
`cavv`/`eciFlag`. I could not reproduce that today. Either the sandbox account's
3DS provisioning changed, or that characterisation was optimistic. Flagging
rather than adjudicating — **3DS should not be signed off until it is re-proven
on a 3DS-enabled Braintree account.**

### C1 — partial capture reports CHARGED, not PartialCharged

Capturing 3000 of an authorized 6000 returns `status: CHARGED`. Review-checklist
item 9 ("partial capture reports `PartialCharged`, not `Charged`", raised on
PR2183) says this should be `PartialCharged` when the captured amount is below
the authorized total. Not fixed here — it needs `resource_common_data.amount` to
carry the authorized total, and the same PR2183 thread records that it is often
`None`. Raised for a reviewer decision.

### C2 — the test harness cannot run Braintree at all (pre-existing)

`ConnectorSpecificConfig::Braintree` has four `repeated string` fields with no
`#[serde(default)]`, so `x-connector-config` fails to parse:
```
INVALID_DATA_FORMAT: Failed to parse X-Connector-Config JSON into ConnectorSpecificConfig
```
All 22 pre-existing Authorize scenarios fail identically on this, masking every
Braintree suite result. Proven by supplying the four fields explicitly, after
which the same request reaches the connector.

Two open PRs already fix it generically — **#1465** and **#1473**. Neither is
merged. Nothing was duplicated here; the live testing above supplies the four
fields in the header instead.

### C3 — raw-card Authorize is not a defect

Raw card now fails fast with an accurate message: Braintree's
`chargeCreditCard` takes a `paymentMethodId`, so a card must be tokenized first.
The real HS path already does this — `config/development.toml:1193 [tokenization]`
carries `braintree`, so HS calls UCS `payment_method_tokenize` before
`payment_authorize`. The harness cannot sequence two calls, which is why its
raw-card scenarios cannot pass. Harness expressiveness, not a connector or HS bug.

## How to reproduce

```bash
export CONNECTOR_AUTH_FILE_PATH="$PWD/creds.json"
cargo build --bin grpc-server
CS__SERVER__PORT=8056 CS__METRICS__PORT=8086 CS__COMMON__ENVIRONMENT=development \
  ./target/debug/grpc-server &
```
Build the config header with the four repeated fields supplied explicitly (until
#1465/#1473 land):
```bash
jq -c '{config:{Braintree:{public_key:.braintree.public_key,
  private_key:.braintree.private_key, merchant_account_id:.braintree.merchant_account_id,
  merchant_config_currency:.braintree.merchant_config_currency,
  apple_pay_supported_networks:[], apple_pay_merchant_capabilities:[],
  gpay_allowed_auth_methods:[], gpay_allowed_card_networks:[]}}}' creds.json
```
Refunds need a settled transaction; the sandbox exposes
`sandboxSettleTransaction(input: {transactionId: ...})` on the root Mutation for this.
