# Hyperswitch-side requirements for a UCS connector

This file is distilled from reading the **Hyperswitch** repository, not from a design document. Every
anchor below was opened at `juspay/hyperswitch` **`main` @ `d8f9262a96`**; paths are HS-repo-relative
unless prefixed `UCS:`. Line numbers are 1-based at that sha and drift — resolve them with
`git -C <hs> show d8f9262a96:<file> | sed -n '<n>p'` before citing one downstream.

> **Why it exists.** A GRACE run integrates the connector into UCS, then meets the HS side hours later
> in end-to-end testing, one failure at a time. In one measured run that ordering cost three plan
> revisions and a `PLAN_CONFLICT`: the routing key, the config arm and two authentication-leg gates were
> each discovered separately, after codegen had already committed to a shape. Reading this list during
> scout costs one pass; rediscovering it costs about a third of a run.
>
> Nothing here is a UCS code change. Every item lands in the **HS** tree — which is why it must be
> enumerated before the plan freezes, not after the first 500.

Two consumers read it. **`grace/workflow/2.1a_hs_scout.md`** Phase 4a walks the mandatory list below and
reports each item present or absent with its anchor. **`grace/workflow/2.3a_plan.md`** §9 turns every
absent mandatory item into an `hs_changes[]` entry. Each item is marked **mandatory** (the connector
cannot transact without it) or **conditional** (with the condition stated). "Checked, condition does not
hold" is an acceptable answer for a conditional item; silence is not.

---

## Index

| Id | Area |
|---|---|
| HU-01 | Routing: is the connector sent to UCS at all |
| HU-02 | Auth mapping: `ConnectorSpecificConfig` and the `x-connector-config` header |
| HU-03 | Per-flow request builders (why a new connector usually needs none) |
| HU-04 | `connector_feature_data`: the only opaque channel, and where it is dropped |
| HU-05 | Dashboard metadata |
| HU-06 | Cypress |
| HU-07 | The UCS client pin |
| HU-08 | Authentication-leg selection: the matrix |
| HU-09 | Authentication legs: per-shape HS edits |
| HU-10 | Authentication legs: state carried between legs |
| HU-11 | Known HS gaps with a per-connector workaround |
| HU-12 | Known HS defects with no per-connector workaround |
| HU-13 | Hard rule: no connector-specific branch in `crates/router` core |
| HU-14 | Config keys: a key's mirror set is per-key, and it is all-or-nothing |
| HU-15 | Recorded facts about the HS PR surface (observations, not fixable items) |

---

### HU-01 — Routing: is the connector sent to UCS at all

**`ucs_enabled` — mandatory, and it is not a toml key.** `check_ucs_availability`
(`crates/router/src/core/unified_connector_service.rs:857`) reads the flag at `:859-860` through
`is_config_flag_enabled(state, consts::UCS_ENABLED)`; the constant is
`crates/router/src/consts.rs:366` (`"ucs_enabled"`). It is a row in the **configs table**, set per
environment, so it does not appear in any `config/*.toml` and cannot be satisfied by a repo edit. A
missing row means every connector takes the Direct path, silently.

**One of two levers, both mandatory-in-the-alternative:**

*(a) `ucs_only_connectors`* — comma-separated, in `config/development.toml:1598` and mirrored in
`config/config.example.toml:1476`, `config/deployments/env_specific.toml:472`,
`config/deployments/integration_test.toml:1053`, `config/deployments/sandbox.toml:1072`,
`config/deployments/production.toml:1061`. Read at `unified_connector_service.rs:889`
(`is_ucs_only = ucs_config.ucs_only_connectors.contains(&connector)`, inside
`determine_connector_integration_type`). The resulting `ConnectorIntegrationType::UcsConnector` arm of
`decide_execution_path:1150` returns UCS **unconditionally** (`:1156-1159`), and
`is_kill_switch_applicable:1137` is `false` for it (`:1141-1144`) — a UCS-only connector cannot be
killed back to Direct, which the doc comment above `:1137` states outright. Note the five mirrors are
**not identical today** (`payconex` is absent from `config.example.toml`, `tesouro` from `sandbox.toml`
and `production.toml`) — a change that edits one and not the others is the normal defect here. See HU-14
for the general rule about which files a key's mirror set covers.

**Omitting it while authentication-leg predicates are overridden has the same failure shape as (ii) in
HU-09, and a reviewer raised it as such.** When the connector is neither in `ucs_only_connectors` nor
reached by a rollout key, `determine_connector_integration_type` returns
`ConnectorIntegrationType::DirectandUCSConnector` (the `else` at `unified_connector_service.rs:903`, and
the `None` arm at `:911` when no UCS config block exists at all). The Direct path resolves each declared
leg to HS's own `ConnectorIntegration` impl for the connector, which for a UCS-authored connector is the
**empty macro-generated one** — so the leg makes no call and returns `Ok(None)` (HU-09 (ii) for the full
chain). Routing and leg enablement are one change, not two: a plan that overrides a leg predicate must
carry the routing entry in the same `hs_changes[]` set.

*(b) Rollout keys* — configs-table rows named
`{UCS_ROLLOUT_PERCENT_CONFIG_PREFIX}{scope}`. Scope built at `:1224`
(`build_merchant_rollout_scope`), candidate keys in precedence order at `:1269`
(the comment there says "the order is load-bearing" — the first key that exists wins), payouts at
`:1296` (`build_rollout_keys_for_payouts`), webhooks a **single flat key** at `:1401`. The value is
`RolloutConfig` JSON — struct at `crates/router/src/core/payments/helpers.rs:2533`, parsed at `:2793`
(`serde_json::from_str::<RolloutConfig>`). **A non-JSON value is not an error and not a default: it
silently means "do not execute".** See HU-12 for the Cypress helper that writes exactly such a value.

**Cache sentinel — read this before creating a key.** An absent key is cached as the literal
`"not_configured"` (`consts.rs:363`, `UCS_ROLLOUT_CONFIG_NOT_CONFIGURED`), written through
`find_config_by_key_unwrap_or` at `helpers.rs:2778`. Creating the key *after* the first lookup therefore
may not take effect until the cache is invalidated — a class of "the config is right but nothing
changed" that costs a test round if it is not expected.

**`[connectors] <name>.base_url` — mandatory even for a UCS-only connector**
(`config/development.toml`, e.g. `:370` for `ilixium`, which is in `ucs_only_connectors`). HS resolves
the base URL before deciding the execution path.

**`ucs_psync_disabled_connectors` — conditional (only when PSync must *not* go to UCS).** Field at
`crates/external_services/src/grpc_client/unified_connector_service.rs:84`, read at
`crates/router/src/core/payments/gateway/psync_gateway.rs:154`, configured at
`config/deployments/env_specific.toml:473` and `config/config.example.toml:1477`.

### HU-02 — Auth mapping: `ConnectorSpecificConfig` and the `x-connector-config` header

**Mandatory, and its absence 500s every single UCS call for the connector.** Two edits in
`crates/router/src/core/unified_connector_service/connector_config.rs`:

1. A variant on `pub enum ConnectorSpecificConfig` (`:221`), carrying the connector's credential fields.
2. A `Connector::<Name> =>` arm in the `match connector` of
   `impl ForeignTryFrom<(Connector, &ConnectorAuthType, Option<&serde_json::Value>)>` — `impl` at
   `:754`, `match` at `:767`, first arm `Connector::Adyen` at `:768`.

The catch-all `_ =>` at `:1902-1908` (under the comment `// --- Unsupported connectors ---`) returns
`ApiErrorResponse::InternalServerError` with `"Connector {} not yet supported for
ConnectorSpecificConfig"`, and `unified_connector_service.rs:2673` calls
`connector_config::build_connector_config_header` on the request path and propagates it. So the failure
mode is not a degraded feature: it is a 500 on authorize, sync, refund, everything.

**Variant and field names are the wire contract.** `UcsConnectorConfig::new` keys the object by
`format!("{:?}", connector)` (`:31`) — the PascalCase `Connector` debug name — and the enum is
externally tagged, so the variant name is the inner key. The result is sent as the
`x-connector-config` header (`build_connector_config_header`, `:1927`; header name
`crates/external_services/src/lib.rs:115`, `UCS_HEADER_CONNECTOR_CONFIG = "x-connector-config"`). The
names must agree with **three** other places: the UCS `message <Name>Config` in
`UCS:crates/types-traits/grpc-api-types/proto/payment.proto`, the per-connector arm in
`crates/router/src/core/connector_validation.rs` (e.g. `Connector::Ilixium` at `:291`), and
`crates/connector_configs/toml/development.toml` `[<c>.connector_auth.<Variant>]` (e.g. `[ilixium]`
`:9174` / `[ilixium.connector_auth.SignatureKey]` `:9175`).

**Record this as a standing risk, not a one-time check:** `ConnectorSpecificConfig` is a
hand-maintained mirror of the UCS-side config enum. Nothing compiles the two against each other, so it
drifts silently and fails only at runtime, as a 500 with an auth-type message.

### HU-03 — Per-flow request builders

**Usually no change — verify, do not assume.** The builders in
`crates/router/src/core/unified_connector_service/transformers.rs` are keyed on the HS **flow type**,
not on the connector. A new connector that reuses existing flows needs **no** new builder. Wired today:

| Flow | Anchor (`transformers.rs`) |
|---|---|
| Authorize | `:567` (`for payments_grpc::PaymentServiceAuthorizeRequest`) |
| CompleteAuthorize | `:809` |
| PSync | `:1158` (`PaymentServiceGetRequest`) |
| Capture | `:2029` |
| SetupRecurring | `:2690` |
| Charge / RepeatPayment | `:2882` (`RecurringPaymentServiceChargeRequest`) |
| Refund | `:7744` |
| RSync | `:7852` (`RefundServiceGetRequest`) |
| Void | `:8067` |
| Pre-authentication leg | `:1832` (`…PreAuthenticateRequest`) |
| Authentication leg | `:1399` (`…AuthenticateRequest`) |
| Post-authentication leg | `:1625` |
| Webhooks | `:7674` (`RequestDetails`), `:7703` (`EventServiceParseRequest`) |

**Conditional (only when the connector uses a value HS does not yet map).** Enum and payment-method
mapping is exhaustive-by-arm and returns `NotImplemented` for anything unmapped:
`build_unified_connector_service_payment_method` (`unified_connector_service.rs:1460`) and its
`NotImplemented` arms; `PaymentMethodType` at `transformers.rs:4180`; `CardNetwork` at `:4327`;
`BankNames` at `:5085`; `Currency` at `:4148`. A new payment method type, card network, bank or
currency is an HS change even though the flow builder is not.

**Mandatory to record as a hard limit: there is no UCS builder for any dispute flow.**
`grep -c 'Dispute' crates/router/src/core/unified_connector_service/transformers.rs` returns **0** at
this sha — no Accept, Defend, SubmitEvidence or dispute-sync request is constructed anywhere on the UCS
path. A dashboard chargeback action therefore cannot reach UCS, whatever the connector implements. Plan
dispute flows as UCS-side only, with no HS reachability.

**Conditional (redirect payment methods other than PayPal).**
`reconstruct_payment_method_data_for_redirect_completion` (`unified_connector_service.rs:1444`) handles
only PayPal, so every other redirect payment method arrives at CompleteAuthorize with no
`payment_method` reconstructed.

### HU-04 — `connector_feature_data`

The one opaque JSON channel that round-trips connector-private state through HS. It is **not** forwarded
uniformly, and the asymmetry is load-bearing for any connector that needs state across two calls.

*Forwarded from router data* (`transformers.rs`): CompleteAuthorize `:1003`, PSync `:1218`, Capture
`:2086`, Void `:8108`, Refund `:7816` (from `connector_metadata`).
*Hard-coded `None`* — the value cannot survive these calls: Authorize `:751` and `:2453`, CreateOrder
`:1091`, SetupRecurring `:2819`, RepeatPayment `:3097`, RSync `:7917`.
*Read back into `connector_metadata`* on the response side at `:3415` and `:3497`.

So a connector may **not** rely on `connector_feature_data` to carry state *into* Authorize or
SetupRecurring. Anything needed there must arrive by another route, or the HS builder must be changed
(an `hs_changes[]` item).

### HU-05 — Dashboard metadata

**Mandatory — without it the dashboard cannot resolve the connector at all**, independent of routing.

- `crates/connector_configs/toml/development.toml`, `sandbox.toml`, `production.toml`: a `[<c>]` table
  plus `[<c>.connector_auth.<Variant>]`, with `[<c>.connector_webhook_details]` **conditional on the
  connector having webhooks**.
- `crates/connector_configs/src/connector.rs`: a `pub <c>: Option<ConnectorTomlConfig>` field (the
  block containing `:328` `fiservcommercehub` and `:335` `givepayments`) **and** the corresponding
  `Connector::<Name> => Ok(connector_data.<c>)` match arm (the block containing `:616`
  `Connector::Fiservcommercehub` and `:726` `Connector::Givepayments`). The field alone does not wire it
  up; both are required.

### HU-06 — Cypress

**Conditional on the run executing Cypress specs** — but then all four parts are required together:

1. `UCS_CONNECTORS` in `cypress-tests/cypress/e2e/configs/Payment/Utils.js:610-615`. This list does
   more than tag: `cypress-tests/cypress/support/commands.js:176-181` flips `executionMode` from
   `"shadow"` to `"primary"` for its members, with the comment "must route `primary` so the classic
   connector is never invoked". A connector absent from it is tested in shadow — i.e. the assertions
   read the **Direct** response.
2. `cypress-tests/cypress/e2e/configs/Payment/<Connector>.js` exporting `connectorDetails`.
3. Its `import` (the block at `Utils.js:6-98`) **and** an entry in the `connectorDetails` map that
   begins at `:99`, resolved through `getConnectorDetails` / `mergeDetails` at `:218`. Import without
   map entry resolves to nothing.
4. A `cypress-tests/creds.json` entry. The file is untracked in a clean checkout, so its absence is
   expected and is not evidence that the entry is unnecessary.

### HU-07 — The UCS client pin

**Conditional on the work adding or changing a UCS proto field — and then mandatory and atomic.** The
`unified-connector-service-client` git tag appears in **three** manifests and must be bumped in all of
them plus `Cargo.lock` re-resolved: `crates/router/Cargo.toml:246-247` (client and `ucs_cards`),
`crates/external_services/Cargo.toml:97`, `crates/hyperswitch_interfaces/Cargo.toml:38` — all
`tag = "2026.09.07.1"` at this sha. Until the bump lands, a new UCS proto field **does not exist** for
HS: the generated types are from the pinned tag, so the field cannot be referenced and a plan that
assumes it will not compile.

### HU-08 — Authentication-leg selection: the matrix

Read this before planning any authentication work. Leg selection is **not** a state machine and **not**
a returned next step.

**There is no `next_authentication_step` in HS `main`.** `grep -rn 'next_authentication_step' --include=*.rs crates`
returns nothing at `d8f9262a96`. Other GRACE files name it; they are describing something that does not
exist. Do not plan against it.

What exists instead: **three independent boolean predicates**, each taking a `CurrentFlowInfo` and
answering only for that call. Trait defaults, all `false`, in
`crates/hyperswitch_interfaces/src/api.rs` —

| Predicate | Trait default |
|---|---|
| `fn is_pre_authentication_flow_required` | `api.rs:463` |
| `fn is_authentication_flow_required` | `api.rs:467` |
| `fn is_post_authentication_flow_required` | `api.rs:471` |

plus the related `fn get_preprocessing_flow_if_needed` at `api.rs:502`. Enum dispatch forwards all
three at `crates/hyperswitch_interfaces/src/connector_integration_interface.rs:582`, `:588`, `:594`.
Because the defaults are `false`, a connector that overrides none takes the plain single-call path —
`worldpay` is the example. Each predicate is answered per call site, so the same connector can return
`true` for `CurrentFlowInfo::Authorize` and `false` for `CurrentFlowInfo::CompleteAuthorize`, or the
reverse; that per-call answer, not any ordering logic in the connector, is what selects the shape.

**Order is fixed in the core, not by the connector.** `crates/router/src/core/payments.rs:5937`,
`:5944`, `:5951` call the three steps in that sequence, each guarded by the running
`should_continue_further`. `:6071` then skips the Authorize call entirely when
`call_connector_service_response.should_continue_further` is `false`.

Two shapes occur in practice:

**Shape A — all legs inside one confirm.** Pre-authentication, then authentication, then Authorize, in
a single `/confirm`. Both of the first two predicates return `true` for `CurrentFlowInfo::Authorize`.
Example: `crates/hyperswitch_connectors/src/connectors/redsys.rs:1123`
(`fn is_authentication_flow_required`).

**Shape B — split across the redirect.** Pre-authentication runs at Authorize; the shopper is
redirected; the remaining legs run at CompleteAuthorize. Examples:
`connectors/nuvei.rs:1801` (`fn is_pre_authentication_flow_required`);
`connectors/nexixpay.rs:1406` (pre) and `:1383` (post — note it answers `false` for
`CurrentFlowInfo::Authorize` and `true` only for `CurrentFlowInfo::CompleteAuthorize` with a card and
`auth_type.is_three_ds()`);
`connectors/moneris.rs:921` (pre), `:935` (post), `:939` (`get_preprocessing_flow_if_needed`);
`connectors/cybersource.rs:2560`; `connectors/paysafe.rs:1374`.

### HU-09 — Authentication legs: per-shape HS edits

**Mandatory whenever the connector overrides any predicate of HU-08, and this is the item runs miss.**
The decision to *continue* after a leg is **hard-coded per connector** in
`crates/router/src/core/payments/flows/authorize_flow.rs`:

- `let should_continue_after_preauthenticate = match connector.connector_name {` at `:556`, arms for
  `Redsys` (`:559`), `Shift4` (`:578`), `Nuvei` (`:579`), `Paysafe` (`:584`), closing `_ => false` at
  `:591`.
- `let should_continue_after_authenticate = match &authorize_router_data.response {` at `:697`, with a
  `Redsys` arm and `_ => false` at `:731`.

**A connector with no arm silently stalls after its leg and never authorizes.** There is no error: the
`_ => false` arm sets `should_continue_further = false`, and `payments.rs:6071` skips Authorize. The
payment simply stops in a non-terminal state. Shape A needs an arm in **both** matches; shape B needs
one only in the first, because the CompleteAuthorize side is generic —
`complete_authorize_flow.rs:424` and `:512` compute `should_continue` from the response, with no
per-connector match.

**Two DISTINCT silent no-ops, and a declared leg needs both cleared.** The `_ => false` above is only
one of them. They fail in opposite directions, so the symptom tells you which you have:

*(i) No `should_continue` arm — the leg runs, the payment stalls.* The connector **is** called; the core
then refuses to proceed because `authorize_flow.rs:591` / `:731` clears `should_continue_further` and
`payments.rs:6071` skips Authorize. Visible as a payment stuck in a non-terminal state.

*(ii) Still listed in a default-implementation macro — the leg does not run, the payment completes.*
Overriding a predicate of HU-08 is only half the change. The predicates live on
`ConnectorSpecifications` (`crates/hyperswitch_interfaces/src/api.rs:453`); the leg's request is built
by a **separate** `ConnectorIntegration` impl, and nothing in the type system links the two — so
overriding the predicate alone compiles cleanly. `crates/hyperswitch_connectors/src/default_implementations.rs`
hands an **empty** `ConnectorIntegration` impl to every connector named in these three invocation lists:

| Macro | Definition | Invocation list |
|---|---|---|
| `default_imp_for_pre_authenticate_steps!` | `:2677` | `:2692-2704` |
| `default_imp_for_authenticate_steps!` | `:2840` | `:2855-3006` |
| `default_imp_for_post_authenticate_steps!` | `:3008` | `:3023-3173` |

An empty impl takes the trait defaults, and the default `fn build_request`
(`crates/hyperswitch_interfaces/src/api.rs:243-252`) increments `metrics::UNIMPLEMENTED_FLOW` (declared
`crates/hyperswitch_interfaces/src/metrics.rs:7`) and returns `Ok(None)`. `execute_connector_processing_step`
then takes `None => Ok(router_data)` (`crates/hyperswitch_interfaces/src/api_client.rs:473`): no HTTP
call, no error, `router_data` returned untouched, and the flow proceeds to Authorize. A reviewer's
`[blocking]` finding stated the consequence exactly — 3-D-Secure card transactions "will appear to
succeed while skipping real authentication". **It looks like success**, which is why no test catches it;
the only signal is the `UNIMPLEMENTED_FLOW` counter.

**Removal is mandatory and mechanical.** For each leg the connector enables, delete its
`connectors::<Name>,` entry from that macro's invocation list and write the real pair in the connector's
own module — `impl Payments<Leg> for <Name>` plus
`impl ConnectorIntegration<<Leg>, Payments<Leg>Data, PaymentsResponseData> for <Name>`. The two cannot
coexist (duplicate impls do not compile), so a connector that has written its own leg impl is
necessarily already out of the list; the dangerous state is the reverse — predicate overridden, list
entry left in place. A connector on the single-call path stays in all three lists, which is correct.
`main` carries a live instance of the wrong combination: one connector still listed at
`default_implementations.rs:3101` also overrides `is_post_authentication_flow_required` to return `true`
for `CurrentFlowInfo::CompleteAuthorize`, so its post-authentication leg resolves to the empty impl.
HU-01 describes a third route to the same `Ok(None)`: correct list removal, but routing that never sends
the connector to UCS.

**Response reshaping arms — conditional on the leg's response needing translation.**
`fn transform_response_for_pre_authenticate_flow` (`authorize_flow.rs:1436`; existing arms
`Connector::Cybersource | Barclaycard` at `:1445`, `Redsys` at `:1503`) and
`fn transform_redirection_response_for_pre_authenticate_flow` (`:1386`). Shape B additionally needs
`complete_authorize_flow.rs:697` (`fn transform_response_for_authenticate_flow`) and `:656`
(`fn transform_redirection_response_for_authenticate_flow`).

### HU-10 — Authentication legs: state carried between legs

The channel is `PaymentsAuthorizeData.ucs_authentication_data`
(`crates/hyperswitch_domain_models/src/router_request_types.rs:132`,
`Option<UcsAuthenticationData>`). Its life in `authorize_flow.rs`: written from the
pre-authentication response at `:527`; copied into the authentication request at `:625`
(`authenticate_request_data.authentication_data = authorize_request_data.ucs_authentication_data.clone()`);
re-read from the authentication response at `:668`; and mirrored into `connector_metadata` as
`{"authentication_data": …}` at `:684-694`, whose comment says why — "This ensures authentication_data
is available in CompleteAuthorize flow". CompleteAuthorize recovers it at `transformers.rs:888-903`.

**Mandatory to account for: the UCS transformers never emit the 3-D-Secure enrollment response
variant.** Both authentication-leg response mappers build `PaymentsResponseData::TransactionResponse`
unconditionally — pre-authentication at `transformers.rs:3321` (impl) / `:3401-3413`, authentication at
`:7217` (impl) / `:7345-7349`; `grep -n 'ThreeDSEnrollmentResponse'` on that file returns nothing. So
`enrolled_for_3ds` and `related_transaction_id` keep their defaults on the UCS path, even though
`authorize_flow.rs:550-553` reads them when the variant *is* produced (which only the Direct path does).
Equivalent state must ride `connector_feature_data` — subject to the HU-04 gaps. Separately,
`related_transaction_id` and `force_3ds_challenge` are **never sent to UCS at all**.

### HU-11 — Known HS gaps with a per-connector workaround or a scoped fix

Each is a real absence; each becomes an `hs_changes[]` item only when the connector's shape needs it.

- **Authentication leg sends no return URL.** `PaymentsAuthenticateData`
  (`router_request_types.rs:955-970`) has no `router_return_url` field, so the builder sends
  `return_url: None` (`transformers.rs:1476`, and `:1600` on the post-authentication leg). Fixing it is
  four edits: the field, both `TryFrom`s (`router_request_types.rs:929` from `PaymentsAuthorizeData`,
  `:972` from `CompleteAuthorizeData`), and the two builder lines. *Conditional on the connector needing
  a return URL on an authentication leg.*
- **`mandate_reference` is hard-coded `Box::new(None)` on both authentication legs** —
  `transformers.rs:3413` (pre) and `:7349` (auth) — although `network_txn_id` is carried through on the
  adjacent lines (`:3430`, `:7352`). A connector that charges on the authenticating leg therefore loses
  its mandate id. *Conditional on the connector returning a mandate id from an authentication leg.*
- **Nested redirect payload values are silently dropped.** The `RedirectionResponse` builder
  (`transformers.rs:5014-5039`) flattens `payload` with `v.as_str().map(...)`, keeping string values
  only; objects, arrays and numbers vanish without a warning. *Conditional on the connector's ACS
  returning non-string values.*
- **An abandoned challenge still triggers CompleteAuthorize with empty params.**
  `crates/router/src/core/payments.rs:4751-4755` defaults the body to `json!({})`
  (`req.json_payload.unwrap_or(serde_json::json!({}))`), so the connector must handle an empty
  redirect payload rather than assume a completed challenge.
- **SetupMandate has a pre-authentication step only.**
  `crates/router/src/core/payments/flows/setup_mandate_flow.rs:348` overrides
  `pre_authentication_step` and nothing else — no authentication or post-authentication override — and
  the SetupRecurring builder pins `enrolled_for_3ds: false` (`transformers.rs:2782`). *Conditional on
  the connector requiring a challenge during mandate setup: that shape is unreachable today.*
- **External 3-D-Secure reaches only the plain Authorize.** `ucs_authentication_data` is populated
  solely from `payment_data.authentication`
  (`crates/router/src/core/payments/transformers.rs:5548-5552`); the merchant-supplied API surface is
  `three_ds_data: Option<ExternalThreeDsData>` (`crates/api_models/src/payments.rs:1512`, struct at
  `:14346`) and does not feed it. *Conditional on the connector supporting merchant-performed
  authentication.*

### HU-12 — Known HS defects with no per-connector workaround

Record these as such. A run must not "fix" them inside the connector, and must not plan a test that
depends on them working.

- **Webhooks reach UCS as GET.** `transformers.rs:7691` sends `method: 1, // POST method for webhooks`,
  but the proto defines `HTTP_METHOD_GET = 1` and `HTTP_METHOD_POST = 2`
  (`UCS:crates/types-traits/grpc-api-types/proto/payment.proto:249-250`). The comment contradicts the
  wire value. Any connector whose signature preimage includes the HTTP method name — grabpay does —
  fails verification on every webhook.
- **The webhook rollout key ignores the kill switch.**
  `unified_connector_service.rs:1416-1427`: `should_call_unified_connector_service_for_webhooks` reuses
  the payment decision logic ("with no call_connector_action to consider") and does not consult the
  kill switch.
- **Shadow mode degrades to Direct with no proxy configured.** `unified_connector_service.rs:1074-1083`
  (`create_updated_session_state_with_proxy`) — a shadow run without a proxy quietly becomes a Direct
  run, so a "shadow comparison" can compare nothing.
- **Cypress `setupUCSConfigs` writes an invalid rollout value.**
  `cypress-tests/cypress/support/commands.js:7930-7943` sets each
  `ucs_rollout_config_{merchantId}_{connector}_card_{Authorize,SetupMandate,PSync}` key to the bare
  string `"1.0"`. That is not `RolloutConfig` JSON (HU-01), so by the parse at `helpers.rs:2793` it
  means "do not execute": specs relying on this helper alone exercise the **Direct** path and their
  passes are not UCS evidence.

### HU-13 — Hard rule: no connector-specific branch in `crates/router` core

**Mandatory, unconditional, and the only rule here that a Hyperswitch maintainer has enforced in writing
against a GRACE-raised PR.** A change that names one connector inside `crates/router` core flow code is
rejected on sight; the review sat as `CHANGES_REQUESTED` and blocked the PR. The maintainer's words:

> we are trying to avoide connector specific code inside core, you can introdice `is_direct_gateway`
> inside function `is_authentication_flow_required`

It was written against a literal `is_<connector> && gateway_context.execution_path.is_direct_gateway()`
condition added inside `crates/router/src/core/payments/flows/authorize_flow.rs`. Nothing of that shape
exists in `main`: the two leg predicate call sites there are connector-agnostic — `:475`
(`is_pre_authentication_flow_required`) and `:609` (`is_authentication_flow_required`), each passing only
a `CurrentFlowInfo`. **Keep them that way.** The test to apply to any core edit: if the diff mentions a
connector by name or by `connector_name` comparison anywhere under `crates/router/src/core/`, it is the
wrong shape. (HU-09's `should_continue` matches are existing per-connector arms; extending an arm list
that already exists is not the same as adding a new connector condition to a generic code path, but it is
equally worth flagging in the PR body so the maintainer decides.)

**The accepted alternative** is to push the decision behind the connector's own trait impl. Two reviewers
asked for it two ways, and both land in the same place:

1. Let the predicate itself consider the execution path. `ExecutionPath::is_direct_gateway`
   (`crates/common_enums/src/enums.rs:2992-2997`; `true` for `Direct` and
   `ShadowUnifiedConnectorService`, `false` for `UnifiedConnectorService`) must be *reachable* from
   inside `is_authentication_flow_required` (`crates/hyperswitch_interfaces/src/api.rs:467`). It is not
   today: `CurrentFlowInfo` (`crates/hyperswitch_domain_models/src/router_request_types.rs:40-77`)
   carries `auth_type`, request data and payment method, and no execution path. So this option is itself
   an HS change — a field on `CurrentFlowInfo`, or a second parameter — and must be planned as one.
2. Extend `ConnectorSpecifications` (`api.rs:453`) with a method that receives the gateway context, so
   the core flow stays connector-agnostic. The context is already a trait: `api::gateway::GatewayContext`
   (`crates/hyperswitch_interfaces/src/api/gateway.rs:33-39`, `fn execution_path()` and
   `fn execution_mode()`), and `authorize_flow.rs:604` already has a
   `gateway_context: &gateway_context::RouterGatewayContext` in scope at the predicate call site
   (`execution_path` field at `crates/router/src/core/payments/gateway/context.rs:44`) — it is simply not
   passed into the predicate. This is the smaller of the two.

Either way the branch lives in the connector's `ConnectorSpecifications` impl under
`crates/hyperswitch_connectors/`, never in the core flow. Plan it as an interface change in
`hs_changes[]`, not as a one-line condition.

### HU-14 — Config keys: a key's mirror set is per-key, and it is all-or-nothing

**Mandatory whenever the work adds the connector to any `[...]_connectors` list key.** A key goes into
**every** file that already carries it, or none. A partial edit is the routine defect: one PR added the
connector to `[network_transaction_id_supported_connectors]` in `config/deployments/integration_test.toml`,
`config/deployments/sandbox.toml` and `config/development.toml`, and missed `config/config.example.toml`
and `config/deployments/production.toml` — a live-environment omission that no build or test catches.

**The set differs per key — derive it, do not reuse another key's list.** Verified at this sha:

| Key | Files carrying it |
|---|---|
| `network_transaction_id_supported_connectors` | `config/config.example.toml:1433`, `config/development.toml:1336`, `config/docker_compose.toml:1231`, `config/deployments/integration_test.toml:314`, `config/deployments/sandbox.toml:321`, `config/deployments/production.toml:314` |
| `ucs_only_connectors` (HU-01) | `config/config.example.toml:1476`, `config/development.toml:1598`, `config/deployments/env_specific.toml:472`, `config/deployments/integration_test.toml:1053`, `config/deployments/sandbox.toml:1072`, `config/deployments/production.toml:1061` |

Six files each, and **not the same six**: `docker_compose.toml` carries the first and not the second;
`deployments/env_specific.toml` carries the second and not the first. So the procedure is a grep, per
key, before editing:

```bash
grep -rln 'network_transaction_id_supported_connectors' config/   # substitute the key
```

Every file it prints is in scope for that key. If a run decides a printed file should *not* get the
entry, say which and why in the PR body — an unexplained gap reads as the defect above.

### HU-15 — Recorded facts about the HS PR surface (observations, not fixable items)

Two things a run will see and must not misread. Neither is something the run can repair in-tree.

- **`pr_linked_issues_check` fails on every GRACE-raised HS PR, and that is expected.** The job is
  `.github/workflows/pr-convention-checks.yml:59` (`name: Verify PR contains one or more linked issues`,
  `:60`). It failed on **8 of 8** HS PRs raised alongside a recent batch of UCS PRs, for one reason:
  GRACE opens no issue. `grace/workflow/2.8_pr_run.md:559-561` already predicts exactly this ("needs a
  linked open issue; GRACE opens none — both PR bodies list it under Required human checks"), and the
  rendered body does carry it, as the `Required human checks` item at `grace/workflow/2.8_pr_run.md:792`.
  Confirmed present. So: a red X on this one job is **not** a run failure and must not trigger an RCA
  loop — but the body item is what makes it a human's to close, so a body that omits it turns an expected
  red X into an unexplained one. Across those 8 PRs no reviewer has ever mentioned the failing job.
- **The Cypress request recorder misclassifies every POST that carries a body — and the defect is GRACE's,
  not Hyperswitch's.** `cypress-tests/cypress/support/graceRecord.js` does **not** exist in HS `main`
  (`ls cypress-tests/cypress/support/` at this sha lists only `commands.js`, `e2e.js`, `mitmProxy.js`,
  `redirectionHandler.js`); it is a GRACE-added test helper that exists only on the run worktrees and the
  PR branches GRACE pushes. The line is
  `: { method: args.length > 1 ? args[0] : "GET", url: args.at(-1) };` — `graceRecord.js:38` in a run
  worktree, e.g. `UCS:grace/runs/<run-id>/hs-wt/cypress-tests/cypress/support/graceRecord.js:38`. For
  `cy.request(method, url, body)` the last positional argument is the **body**, so the body is recorded
  as the URL, `flowFor(opts.method, opts.url)` matches nothing, and the call is dropped from the record
  instead of being classified. Raised three times across two PRs and still unfixed, so a run must treat
  the recorded flow set as **incomplete for POSTs with a payload** rather than as evidence a flow was
  never exercised. Fixing it is a change to GRACE's own helper generation, not an `hs_changes[]` item.

---

## How to refresh

Re-verify when the HS pin moves or when a run reports an anchor that does not resolve. The anchors are
line numbers in fast-moving files; a stale one is a wrong claim, not a rounding error.

```bash
HS=/path/to/hyperswitch; SHA=<hs sha>
while IFS=: read -r f l; do
  printf '%-78s %s\n' "$f:$l" "$(git -C "$HS" show "$SHA:$f" | sed -n "${l}p")"
done <<'EOF'
crates/router/src/core/unified_connector_service.rs:889
crates/router/src/core/unified_connector_service/connector_config.rs:767
crates/router/src/core/payments/flows/authorize_flow.rs:556
crates/router/src/core/payments/flows/authorize_flow.rs:697
crates/hyperswitch_interfaces/src/api.rs:463
crates/hyperswitch_connectors/src/default_implementations.rs:2840
crates/hyperswitch_connectors/src/default_implementations.rs:3008
crates/hyperswitch_interfaces/src/api_client.rs:473
crates/hyperswitch_domain_models/src/router_request_types.rs:40
crates/hyperswitch_interfaces/src/api/gateway.rs:33
EOF
```

Rules for editing this file:

- Every claim carries a `file:line` that resolved when it was written, and names the sha it resolved at.
- Mark each item **mandatory** or **conditional**, and state the condition. An unmarked item is a bug
  in this file: the planner cannot classify it.
- Never assert an absence from a name grep alone. Cite the exact command that returned nothing, as
  HU-03 and HU-08 do.
- Keep ids stable. An item that stops applying is struck through with a one-line reason and keeps its
  `HU-NN`; scout and the planner cite these ids in stored artifacts.
