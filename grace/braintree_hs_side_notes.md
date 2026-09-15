# Braintree HS-side notes (recon for the HS → UCS → Braintree E2E path)

HS checkout: /home/infamous/hyperswitch1 @ `main` a2978004a4.

## What already exists on the HS side — no code needed

`crates/router/src/core/unified_connector_service/connector_config.rs`
already carries Braintree end to end:

- `BraintreeMetadata` (:48) — `merchant_account_id`, `merchant_config_currency`,
  `apple_pay_supported_networks`, `apple_pay_merchant_capabilities`,
  `gpay_allowed_auth_methods`, `gpay_allowed_card_networks`.
- `UnifiedConnectorServiceConfig::Braintree` variant (:235).
- `Connector::Braintree` arm (:832): requires `ConnectorAuthType::SignatureKey`,
  maps `api_key -> public_key`, `api_secret -> private_key`, drops `key1`, and
  **requires** a metadata block (errors otherwise).

That matches the UCS proto `BraintreeConfig`
(`crates/types-traits/grpc-api-types/proto/payment.proto:4996`) field for field,
so no proto or mapping change is needed for this scope.

`crates/external_services/src/grpc_client/unified_connector_service.rs` already
exposes every RPC this work needs:

| Requirement | HS UCS client method | line |
|---|---|---|
| CIT / auto capture / $0 auth via authorize | `payment_authorize` | 810 |
| PSync | `payment_get` | 847 |
| Manual capture, partial capture | `payment_capture` | 882 |
| $0 auth (zero-amount verification) | `payment_setup_recurring` | 917 |
| MIT / NTID | `recurring_payment_charge` | 951 |
| Void | `payment_void` | 989 |
| 3DS | `payment_pre_authenticate` 661 / `payment_authenticate` 698 / `payment_post_authenticate` 773 |
| Refund / partial refund | `payment_refund` | 1094 |
| RSync | `refund_get` | 1128 |
| Webhooks (payments, refunds, chargebacks) | `incoming_webhook_parse_event` 1064 / `incoming_webhook_handle_event` 1024 |

## The one HS change this work needs — a config-only PR

`should_call_unified_connector_service`
(`crates/router/src/core/unified_connector_service.rs:916`) gates Direct vs UCS.
Braintree is **not** in `ucs_only_connectors` (`config/development.toml:1598`).

Two ways to route Braintree through UCS, both config:

1. **`ucs_only_connectors`** — add `braintree` to the comma-separated list in
   `config/development.toml`, and to `sandbox.toml` / `production.toml` when it
   ships. This is the dedicated HS PR.
2. **Rollout percentage** — no code, no PR; set a `config`-table row for a local
   run. `build_rollout_keys_by_precedence` (:1270) resolves, lowest→highest:
   `ucs_rollout_config_{org_id}`,
   `ucs_rollout_config_{org_id}_{merchant_id}`,
   `ucs_rollout_config_{org_id}_{scope}`,
   `ucs_rollout_config_{scope}`,
   where `scope` = `{merchant_id}_{connector}_{flow}` for `Execute`/`RSync` and
   `{merchant_id}_{connector}_{flow}_{payment_method}` otherwise
   (`build_merchant_rollout_scope`, :1224). Prefix const:
   `crates/router/src/consts.rs:359`. Value `100` = all traffic to UCS.

Webhooks are gated separately by
`should_call_unified_connector_service_for_webhooks` (:1385), called from
`crates/router/src/core/webhooks/incoming.rs`.

Use (2) to prove the flows locally; raise (1) as the shipping PR.

## The raw-card tokenization question — resolved, HS is already wired

The Authorize run flagged that raw-card Authorize now requires a payment method
token, and that `PaymentsAuthorizeData` has no `payment_method_token` field
(`PaymentFlowData.payment_method_token` was removed — see the dead-code note at
`domain_types/src/router_data.rs:4635`). That reads like an E2E blocker. It is not,
on the HS path:

- HS `config/development.toml:1193 [tokenization]` already carries
  `braintree = { long_lived_token = false, payment_method = "card,wallet" }`.
- `decide_payment_method_tokenize_action` (`crates/router/src/core/payments.rs:9366`)
  therefore returns `TokenizeInConnectorAndRouter` for a Braintree card with no
  parent token, or `ConnectorToken(..)` on a Redis hit
  (`pm_token_{token}_{payment_method}_{connector}`).
- `crates/router/src/core/payments/gateway/payment_method_token_create_gateway.rs:125`
  then calls UCS `payment_method_tokenize` **before** `payment_authorize`, and the
  resulting nonce reaches Authorize as
  `PaymentMethodData::PaymentMethodToken(..)` — the dispatch arm the Authorize run
  fixed.

So the real HS → UCS → Braintree card path is: **Tokenize → Authorize**, and it is
already wired on both sides. No HS code change, no proto change.

What genuinely cannot pass is the **integration-tests harness**, which drives
`PaymentService/Authorize` directly with raw card data and has no way to sequence a
`PaymentMethodService/Tokenize` call ahead of it. That is a harness-expressiveness
limitation in `connector_specs/braintree/specs.json`, not a connector or HS defect,
and it predates this work. Prove the card path via HS, and via a two-step grpcurl
(Tokenize then Authorize), rather than via the raw-card scenario.

## The HS PR IS required — and it is more than a config line

Earlier in this file I recorded the HS change as "config only". That was true for the
non-3DS flows. It is **not** true for 3DS. Two separate HS changes are needed.

### 1. Routing Braintree to UCS — config

`braintree` added to `ucs_only_connectors` (`config/development.toml:1598`), or a
`ucs_rollout_config_*` row for a local run. See above.

### 2. Making the three 3DS legs reachable — code

HS decides whether to call UCS `PreAuthenticate` / `Authenticate` / `PostAuthenticate`
from three trait methods on the **HS-side connector impl**, not from the UCS
implementation and not from config:

| Leg | HS call site | Gate |
|---|---|---|
| PreAuthenticate | `core/payments/flows/authorize_flow.rs:1808` (entered from `pre_authentication_step`, :465) | `is_pre_authentication_flow_required(CurrentFlowInfo::Authorize { auth_type, request_data })` |
| Authenticate | `core/payments/flows/complete_authorize_flow.rs:822, :981` | `is_authentication_flow_required(..)` |
| PostAuthenticate | `core/payments/flows/complete_authorize_flow.rs:1087, :1230` | `is_post_authentication_flow_required(..)` |

All three default to `false` in `crates/hyperswitch_interfaces/src/api.rs:463, :467, :471`.

**`crates/hyperswitch_connectors/src/connectors/braintree.rs` overrides none of them.**
Eleven connectors do — `barclaycard`, `cybersource`, `moneris`, `nexixpay`, `nmi`,
`nuvei`, `paysafe`, `redsys`, `shift4`, `worldpayxml`, `ilixium` — Braintree is not
among them.

So with UCS's 3DS legs implemented and Braintree routed to UCS, HS will still never
call them: `pre_authentication_step` short-circuits and Authorize runs straight
through. **The 3DS work is unreachable end to end until this HS PR lands.**

Reference shape, `barclaycard.rs:1640-1700`:

- `is_pre_authentication_flow_required` → `Authorize { auth_type, request_data }` ⇒
  `auth_type == AuthenticationType::ThreeDs && request_data.is_card()`; every other
  `CurrentFlowInfo` arm ⇒ `false`.
- `is_authentication_flow_required` → `CompleteAuthorize { request_data, .. }` ⇒ true
  when `redirect_response.params` is present and non-empty (the challenge came back).
- `is_post_authentication_flow_required` → the exact complement: `CompleteAuthorize`
  with absent/empty params ⇒ true.

Braintree's topology matches that split: PreAuthenticate does DDC
(`createClientToken` + `tokenizeCreditCard`), the challenge returns through the
redirect, and the completion leg resolves to `performThreeDSecureLookup`.

### Scope of the HS PR

- `crates/hyperswitch_connectors/src/connectors/braintree.rs` — the three overrides.
- `config/development.toml` — `braintree` into `ucs_only_connectors` (and
  `sandbox.toml` / `production.toml` when it ships).
- No proto change, no `connector_config.rs` change — `BraintreeMetadata` and the
  `Connector::Braintree` auth arm already match the UCS `BraintreeConfig` message.

## Two independent drivers for the 3DS legs — both are needed, they are not alternatives

The PostAuthenticate run added a `next_authentication_step` override to the UCS
Braintree connector and reported that "without it all three legs were unreachable".
That is true — but only for one of the two callers. Do not read it as making the HS
trait overrides unnecessary. They drive different paths.

### Path A — UCS composite service (what `next_authentication_step` serves)

`crates/internal/composite-service/src/payments.rs:819` runs an authentication loop
*inside UCS*:

```
loop {
    let next_step = connector_data.connector.next_authentication_step(
        auth_type, payment_method, redirect_state, state.completed_step);
    match next_step { PreAuthenticate => .., Authenticate => .., PostAuthenticate => .., Authorize => break }
}
```

`AuthenticationStep` and the trait default live at
`crates/types-traits/interfaces/src/connector_types.rs:74` and `:276`; the default
returns `AuthenticationStep::Authorize`, i.e. skip 3DS entirely. The Braintree override
is what makes the composite caller walk the trio. This path serves direct UCS callers,
the SDKs, and the integration-test harness.

### Path B — HS granular per-leg calls (what the HS trait overrides serve)

HS does **not** use the composite RPC. It calls each leg itself:
`authorize_flow.rs:1808` → `payment_pre_authenticate`;
`complete_authorize_flow.rs:822/:981` → `payment_authenticate`;
`complete_authorize_flow.rs:1087/:1230` → `payment_post_authenticate`.
Each is gated by the HS-side `is_pre_authentication_flow_required` /
`is_authentication_flow_required` / `is_post_authentication_flow_required`, which
default to `false` and which HS's `braintree.rs` does not override.

### Consequence

- A UCS-native or SDK caller gets the full 3DS trio today, from the UCS override alone.
- **HS still gets no 3DS for Braintree** until the HS PR lands. The UCS override is
  invisible to HS's granular path — different trait, different crate, different caller.

Both changes ship, or 3DS works for everyone except Hyperswitch.

## Smart retry / GSM — this one cannot ship as a PR at all

"Smart retry enablement with GSM error code update" is **runtime data, not code**.

GSM rows live in the Postgres table `gateway_status_map`. The migrations
(`migrations/2023-11-07-110139_add_gsm_table` and five later ones) create and extend the
*table*; they seed no rows, and no connector's rows are checked into the repo for any
connector. Rows are created through the admin API (`crates/router/src/routes/gsm.rs`,
`POST /gsm`), shape per `cypress-tests/cypress/fixtures/gsm-body.json`:

```json
{ "connector": "braintree", "flow": "Authorize", "sub_flow": "sub_flow",
  "code": "<connector error code>", "message": "<connector error message>",
  "status": "failure", "decision": "retry", "step_up_possible": false }
```

Columns that matter for retry behaviour (`crates/diesel_models/src/gsm.rs:27`):
`decision` (`GsmDecision::Retry` | `DoDefault`, `common_enums/src/enums.rs:390`),
`step_up_possible`, `clear_pan_possible`, `error_category`
(`ErrorCategory`, `:10928` — `FrmDecline`, `ProcessorDowntime`,
`ProcessorDeclineUnauthorized`, `IssueWithPaymentMethod`,
`ProcessorDeclineIncorrectData`, `HardDecline`, `SoftDecline`), plus
`unified_code` / `unified_message`.

`ErrorCategory::should_perform_elimination_routing` (`:10939`) returns true only for
`ProcessorDowntime` and `ProcessorDeclineUnauthorized` — so the category chosen per code
also decides whether elimination routing kicks in, not just whether a retry happens.

### What this means for delivery

There is no PR to raise. The deliverable is a **GSM row set** for
`connector = "braintree"`, one row per Braintree processor response code, with a
soft/hard decline judgement per code driving `decision` and `error_category`. Produce it
as an applyable artifact (curl script / JSON) against the official Braintree processor
response code list, and hand it to whoever owns the HS database.

The UCS side of the dependency is already satisfied: the Authorize run wired
`network_advice_code` / `network_decline_code` / `network_error_message` from Braintree's
processor response, so the codes GSM keys off now actually reach HS. Hyperswitch's own
Braintree hardcodes all three to `None` at every error site, so this data has no value on
the direct (non-UCS) path — another instance of UCS leading the reference.
