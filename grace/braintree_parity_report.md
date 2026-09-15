# Braintree — UCS ↔ hyperswitch logic-parity report

**Scope**: `Authorize` flow, `Card` payment method (plus the Authorize error/response path and
the 3DS branches that feed Authorize). Refund, RSync, Capture, Void, PSync, webhooks and
payouts are out of scope except where they share the Authorize error builder.

**Compared**

| Side | Files |
|---|---|
| UCS (this repo, `feat/braintree_grace`) | `crates/integrations/connector-integration/src/connectors/braintree.rs`, `crates/integrations/connector-integration/src/connectors/braintree/transformers.rs` |
| hyperswitch reference | `/home/infamous/hyperswitch1/crates/hyperswitch_connectors/src/connectors/braintree.rs`, `.../braintree/transformers.rs` (`main` @ a2978004a4) |

**Headline** — before this run the two implementations were near-clones: every card-authorize
GraphQL query string was byte-identical, the status map was byte-identical, and the 3DS
pass-through mapping was identical. The comparison surfaced **three real UCS defects** (two
blocking) and confirmed that **most of the enrichment brief is not a parity gap at all** —
hyperswitch does not implement those fields either, so UCS is now deliberately ahead.

---

## 1. Divergences that were UCS bugs — FIXED in this run

### 1.1 Card Authorize was unreachable (blocking)

Independently verified, not merely reported:

- the Authorize dispatch routed `PaymentMethodData::Card(_)` into `CardPaymentRequest::try_from`;
- `CardPaymentRequest::try_from` unconditionally required `PaymentMethodData::PaymentMethodToken`
  and otherwise returned `MissingRequiredField("payment_method_token")`;
- `PaymentMethodData::PaymentMethodToken(_)` sat in the same dispatch's `NotSupported` arm.

So a raw card died in the builder and a token died in the dispatch: **no input could produce a
successful card authorize.**

*Root cause of the divergence*: hyperswitch keeps the Braintree token in a **separate**
`RouterData.payment_method_token` field (`get_payment_method_token()?`) while
`payment_method_data` stays `Card`. UCS's `PaymentsAuthorizeData` has no such field, and the
port collapsed both into the single `payment_method_data` enum.

*Fix*: the tokenized path is the real one — `chargeCreditCard` / `authorizeCreditCard` take
`paymentMethodId: ID!`, and there is no tokenize→authorize sequencing anywhere in UCS
(`composite-service/payments.rs::process_composite_authorize` never calls
`PaymentMethodService/Tokenize`). `PaymentMethodToken(_)` now routes into `CardPaymentRequest`
and is out of the `NotSupported` arm; a raw `Card(_)` returns an accurate, actionable
`NotSupported` with a fully populated `IntegrationErrorContext` telling the caller to tokenize
first, instead of the misleading `MissingRequiredField`.

*Deliberately not done*: adding `payment_method_token` to `PaymentsAuthorizeData` to match the
HS shape. That is a core-type + proto change, outside a connector-scoped run. **It is the right
long-term fix** and is the precondition for the two items in §4.

### 1.2 The hosted-3DS redirect branch was self-contradictory (blocking)

Same root cause. The branch needed `payment_method_data` to be `PaymentMethodToken` (to read
the token) while `get_braintree_redirect_form` → `get_card_isin_from_payment_method_data` needed
it to be `Card` (to read the BIN). One enum value cannot satisfy both, so the path made a
pointless `createClientToken` round-trip and then died in the response transformer.

*Fix*: it now fails **before** the HTTP call with
`FlowNotSupported { flow: "Braintree-hosted 3D Secure authorize" }`, explaining that
CompleteAuthorize is unimplemented and that external-MPI 3DS is the supported route. The
unconstructible `BraintreePaymentsRequest::CardThreeDs` variant was removed.

### 1.3 `paymentInitiator` dropped on the vaulting authorize

HS's `VaultTransactionBody` carries `payment_initiator` and sets `RECURRING_FIRST` when vaulting.
UCS's omitted the field entirely and its `PaymentInitiatorType` enum had only `Unscheduled`, so a
vaulting card authorize told Braintree nothing about its CIT/recurring-first nature.

*Fix*: added the `RecurringFirst` variant and the `payment_initiator` field. Proven on the wire —
`"paymentInitiator":"RECURRING_FIRST"` → `CHARGED` + a `mandateReference`.

---

## 2. Where the brief's premise was wrong — UCS now deliberately LEADS hyperswitch

The enrichment brief framed these as UCS gaps. They are not: **hyperswitch does not implement
them either.** UCS is now ahead of the reference on all of them, which is a deliberate,
documented divergence rather than a bug on either side.

| Area | hyperswitch | UCS after this run |
|---|---|---|
| `network_advice_code` / `network_decline_code` / `network_error_message` | Hardcoded `None` at **all 9** Braintree error-construction sites (`braintree.rs:186-188, 206-208`; `transformers.rs:752-754, 822-824, 913-915, 993-995, 1060-1062, 1129-1131, 1204-1206`). The source fields are not even in its GraphQL selection set. | Populated from `merchantAdviceCodeResponse.code` → advice, `networkResponse.code` → decline, `networkResponse.message` → error message. Proven on a forced sandbox decline: advice `01`, decline `XX`. HS GSM smart-retry can now key off them. |
| AVS response codes | Zero hits across all four files — not selected, not deserialized, not surfaced. | `avsStreetAddressResponseCode` / `avsPostalCodeResponseCode` surfaced on `AdditionalPaymentMethodConnectorResponse::Card{payment_checks}`, the same slot BoA/Cybersource use. |
| CVV response code | Zero hits. | `cvvResponseCode` surfaced in the same slot. |
| Dynamic descriptor | Not sent. | `descriptor { name, phone, url }` with Braintree's length rules. |
| L2/L3 | Not sent. | `tax{taxAmount,taxExempt}`, `discountAmount`, `shipping{shippingAddress,shippingAmount}`, `lineItems[]`. |
| Billing / shipping `AddressInput` | Not sent. | Full PayPal-style `AddressInput` + `PhoneInput`, alpha-3 `countryCode`. |
| `riskData`, `externalVault` | Not sent. | Still not sent — not in the brief. |
| External 3DS pass-through | Implemented, identical mapping. | Corrected to the SDL names and extended (see §3.1). |

**Consequence worth flagging**: because UCS now selects response fields hyperswitch never
selected, the enlarged selection sets were gated on empirical verification against the pinned
`Braintree-Version: 2019-01-01` (introspection **and** live success/decline mutations). Selecting
a field the versioned schema does not expose is a hard GraphQL validation error that would break
**every** Authorize. Every added field was confirmed exposed; nothing shipped unverified.

---

## 3. Divergences that are deliberate

### 3.1 External 3DS field names — the brief and the REST docs disagree with the SDL

The SDL is authoritative and UCS follows it. Recorded here because the wrong names are easy to
reintroduce from the REST/SDK documentation:

| Wrong (REST/SDK/brief) | Correct (GraphQL SDL) |
|---|---|
| `xid` | `xId` |
| `threeDSecureVersion` | `version` |
| `authenticationResponse` | `directoryServerResponse` |
| `dsTransactionId` | `directoryServerTransactionId` |

`dsTransactionId` **does** exist in the SDL, but on `ThreeDSecurePriorAuthenticationDetailsInput`
under the 3RI / `performThreeDSecureLookup` tree — a different feature. UCS does not emit it on
the Authorize pass-through, and a unit test asserts the wrong names are absent.

Also: `eciFlag` is `ECommerceIndicator!` (non-null), so the whole `passThrough` object is omitted
when no ECI is present rather than sent with a null. `xId` is emitted on 3DS1 only. `cavvAlgorithm`
and `network` are modelled but have no UCS source field.

### 3.2 Structural differences carried over from the port

- `MandatePayment` is handled inside Authorize in HS vs UCS's first-class `RepeatPayment` flow.
- HS's `Session` variant vs UCS's `ClientAuthenticationToken` flow.
- Config source: HS `connector_meta_data` vs UCS `ConnectorSpecificConfig`.
- UCS's `typed_connector_response` / `raw_connector_response` observability fields vs HS's
  `connector_response_reference_id` / `connector_metadata` on `ErrorResponse`.
- `cavv` optionality — a domain-type difference with identical wire output.
- HS-only `TOKENIZE_NETWORK_TOKEN` (wallet decrypt, out of card scope).

### 3.3 Status mapping — zero divergences

The UCS map is identical to HS's, including every terminal state
(`PROCESSOR_DECLINED`, `GATEWAY_REJECTED`, `FAILED`, `AUTHORIZATION_EXPIRED`,
`SETTLEMENT_DECLINED`, `VOIDED`). Two **shared** modelling choices are worth recording as
known risks rather than bugs, since changing either unilaterally would diverge from HS:

- `SETTLEMENT_PENDING` → `Charged` is optimistic.
- `SETTLEMENT_DECLINED` → `Failure` reports a payment failure for a settlement-stage decline.

Reviewer-checklist item 7 ("a terminal connector state must map to a terminal UCS state") is
satisfied: no terminal Braintree state maps to `Pending`.

---

## 4. Known gaps left open (not regressions)

1. **`PaymentsAuthorizeData` has no `payment_method_token` field.** Until it does, the
   `no3ds_*_credit_card` / `threeds_*` Authorize scenarios in
   `crates/internal/integration-tests/src/connector_specs/braintree/` cannot pass — they send a
   raw card. They could never pass before either (they hit the `MissingRequiredField` dead end),
   so this is not a regression; they now fail fast with an accurate message.
   `cargo run --bin check_connector_specs` still reports `All checks passed. OK.`
2. **No CompleteAuthorize flow.** HS has the full integration; UCS has none, and
   `BraintreeCompleteAuthResponse`, `BraintreeCompleteChargeResponse`, `BraintreeThreeDsResponse`,
   `BraintreeThreeDsErrorResponse` and `BraintreeRedirectionResponse` are ported-but-dead types.
   Left in place as the CompleteAuthorize scaffolding rather than widening this diff.
3. **`purchaseOrderNumber` has no UCS source** — zero hits for `purchase_order` across the domain
   types and the proto tree. This blocks Level 2 qualification and needs a proto field first.
4. **`shipsFromPostalCode` is unreachable** — the code reads `AddressDetails.origin_zip`, but the
   proto `Address` message has no `origin_zip` field. Required for Level 3.
5. **`descriptor.url` has no source** — `BillingDescriptor` has no url field; the struct member
   exists so the mapping is a one-liner once a source does.
6. **`BraintreePaymentStatus` derives `strum::Display` without `serialize_all`**, so
   `status.to_string()` yields `"ProcessorDeclined"`, not `"PROCESSOR_DECLINED"`. It leaks into
   `create_failure_error_response` for PSync/Capture/Void/Refund/RepeatPayment. Not changed here
   because it would alter error codes across six flows. **Recommended follow-up.**
7. **`l2_l3_data.order_info.order_details` does not reach `PaymentFlowData` on Authorize** — line
   items sent only under `l2_l3_data` produced no `lineItems`; the top-level `order_details` field
   works. The connector reads both (top-level first), so this is a framework mapping gap, not a
   Braintree bug.
8. **`RepeatPayment` shares `CHARGE_CREDIT_CARD_MUTATION`** and so now receives the enriched
   response, but its failure branch still uses the old `create_failure_error_response`. A 3-line
   upgrade, left out to keep the diff Authorize-scoped.
9. **Framework-level, all connectors**: HS's `is_auto_capture()` returns `Result` and rejects
   `ManualMultiple`/`Scheduled`; UCS's returns `bool` and silently treats them as manual, so such a
   request becomes `authorizeCreditCard` instead of being rejected. Documented rather than patched
   in Braintree.

---

## 5. Appendix — reviewer-checklist audit

`grace/braintree_review_checklist.md` (22 recurring reviewer issues mined from the last 20
merged PRs) was audited against the final diff by an independent pass that re-derived every
claim from the code rather than trusting the implementing agent's self-report. It overturned
four of that self-report's claims. All four are now **fixed**:

| Item | Finding | Fix |
|---|---|---|
| 6 — unknown status → `Unspecified` | `BraintreePaymentStatus` had no `#[serde(other)]` and the `AttemptStatus` map was exhaustive, so **an unrecognised Braintree status failed deserialization of the entire Authorize response**. Pre-existing, not introduced by this run, but squarely applicable. | Added an `Unknown` variant with `#[serde(other)]`, mapped to `AttemptStatus::Unspecified`, and mapped it to non-terminal `PostCaptureVoidStatus::Pending` in the post-capture-void match so an unknown status cannot invent a void failure. New test `an_unrecognised_braintree_status_is_unspecified_not_an_invented_terminal_state`. |
| 15 — reuse existing helpers | The descriptor-name truncation hand-rolled `Secret::new(name.expose().chars().take(22)…)`. | Replaced with the existing `utils::truncate_secret_string(name, 22)`, which also uses `.peek()` instead of `.expose()`. |
| 17 — no `billing_full_name` fallback for cardholder name | Two tokenize sites did `get_optional_billing_full_name().unwrap_or(Secret::new("".to_string()))`. Pre-existing — but this run makes tokenization **mandatory** for every Braintree card payment (§1.1), so the empty-string fallback moved from an optional side path onto 100% of card traffic. | `cardholder_name` now comes from `card_data.card_holder_name` explicitly and is `Option<Secret<String>>` with `skip_serializing_if`, so it is omitted rather than sent as an empty string Braintree would store verbatim. New test `cardholder_name_comes_from_the_card_and_is_omitted_when_absent`. |
| 18 — populated `IntegrationErrorContext` | One new error construction used `context: Default::default()` — the L2/L3 amount converter. | Populated with `additional_context`, `suggested_action` and a `doc_url` to Braintree's L2/L3 reference. |

Items 1, 2, 3, 4, 5, 10, 12, 13, 14, 16, 19, 20, 22 were confirmed satisfied against the code.
Items 7, 8, 9, 11, 21 are not applicable to this diff. **Item 14 — the highest-stakes one, since
PR2187 cites this very file as the repo-wide reference implementation for preferring the
per-request field over the connector-config copy — was verified intact at both sites** and sits
outside every modified hunk.

### Residual risks recorded by the audit

- **Response selection set vs the `Braintree-Version: 2019-01-01` pin.** Deserialization
  tolerance is confirmed good: every added response field is `Option<…>` with `#[serde(default)]`,
  and three tests prove a response omitting them still parses. The GraphQL *validity* of the
  enlarged selection under the pin was proven empirically — live sandbox calls returned populated
  `processorAuthorizationResponse`, `statusHistory`, `merchantAdviceCodeResponse` and
  `networkResponse` on both a success and a forced decline — but **that proof is not checked in**.
  Recommend capturing it as a fixture (ART/deja recording) before merge, since a field absent from
  the pinned schema is a hard validation error that would fail every Authorize.
- `statusHistory` is assumed reverse-chronological when picking the event carrying decline detail.
  If Braintree returns it oldest-first, a multi-event transaction could report a stale event. The
  `terminal` flag is selected and modelled but not yet read, so it cannot currently disambiguate.
- `build_transaction_enrichment` itself has no direct test — the per-request-vs-`l2_l3_data`
  precedence, the reconciliation gating that drops the whole L2/L3 block, the 249-line-item cap
  and the `non_zero` filter are covered only through their leaf helpers.

---

## 6. 3DS

Scope of this section: the **`PreAuthenticate`** leg only (Braintree-hosted 3D Secure, leg 1).
`Authenticate` and `PostAuthenticate` remain `not_implemented` stubs
(`crates/integrations/connector-integration/src/connectors/braintree.rs`,
`macros::macro_connector_flow_status_impls!`) and are separate runs on this branch. They are
described here only far enough to show that this leg does not consume their territory.

### 6.1 Topology: UCS's three legs vs hyperswitch's `CompleteAuthorize`

The two trees do not disagree about *what Braintree does*. They disagree about **where the
browser round-trip is expressed in the flow vocabulary**, because the vocabularies differ:

- hyperswitch has `CompleteAuthorize`. Braintree's completion leg is
  `impl ConnectorIntegration<CompleteAuthorize, CompleteAuthorizeData, PaymentsResponseData> for
  Braintree` at
  `/home/infamous/hyperswitch1/crates/hyperswitch_connectors/src/connectors/braintree.rs:1237`,
  enabled by the marker `impl api::PaymentsCompleteAuthorize for Braintree` at `:227`, with
  `impl ConnectorRedirectResponse for Braintree` at `:1191` deciding what a browser post-back
  means (`PaymentAction::CompleteAuthorize | PaymentAuthenticateCompleteAuthorize =>
  CallConnectorAction::Trigger` at `:1228-1231`; a payload on `PaymentAction::PSync` is the
  failure path and yields `AttemptStatus::AuthenticationFailed` at `:1221`).
- **UCS has no `CompleteAuthorize` flow marker at all** — `connector_flow.rs` declares none and
  `FlowName` has no such member. The UCS 3DS vocabulary is exactly
  `PreAuthenticate` / `Authenticate` / `PostAuthenticate`.

Verified mapping (every hyperswitch line below was re-read in
`/home/infamous/hyperswitch1` for this run, not copied from an earlier note):

| Braintree-side step | hyperswitch | UCS after this run |
|---|---|---|
| Mint a card nonce | Inside the Authorize builder — `PaymentMethodData::Card(_)` is tokenized implicitly by the SDK, and the server never calls `tokenizeCreditCard` on the 3DS branch | Explicit `PaymentMethodToken` flow (`tokenizeCreditCard`), `braintree.rs`, already implemented — **and** re-issued as a root field of this leg's document (§6.3) |
| Mint a client token (DDC bootstrap) | The 3DS branch of the **Authorize** request builder: `BraintreePaymentsRequest::CardThreeDs(BraintreeClientTokenRequest::try_from(metadata)?)`, guarded by `is_three_ds() && authentication_data.is_none()` — `hyperswitch1/.../braintree/transformers.rs:415-424` | **`PreAuthenticate`** — this run |
| Hand `client_token` + `card_token` + `bin` to the browser | `RedirectForm::Braintree { client_token, card_token, bin, acs_url }` (`hyperswitch1/crates/hyperswitch_domain_models/src/router_response_types.rs:438-443`), rendered into a maud page that loads `https://js.braintreegateway.com/web/3.97.1/js/three-d-secure.js` and calls `threeDs.verifyCard({...})` (`hyperswitch1/crates/router/src/services/api.rs:1283`, `:1317`, `:1331`) | The same `RedirectForm::Braintree`, surfaced as `PreAuthenticateResponse.redirection_data` (§6.3) |
| Perform the 3DS lookup | **Client-side.** `onLookupComplete` is a Braintree JS SDK callback (`services/api.rs:1331`); hyperswitch's server never issues a lookup | **`Authenticate`** — out of scope, will use `performThreeDSecureLookup` server-side (§6.2) |
| Consume the 3DS-verified nonce and charge | `CompleteAuthorize` (`braintree.rs:1237`), reading `payload.nonce` out of the browser's `authentication_response` | `PostAuthenticate` then `Authorize` — out of scope |

Two consequences worth recording:

1. **hyperswitch's `acs_url` is not an ACS URL.** It is hyperswitch's own
   `complete_authorize_url`, which the generated page posts the SDK result back to. UCS inherits
   the field name verbatim (`router_response_types.rs`, `RedirectForm::Braintree`), so this leg
   fills it from `continue_redirection_url`, falling back to `router_return_url`. The misnomer is
   commented at the call site so the next reader does not send an issuer URL there.
2. **UCS's split is strictly finer.** hyperswitch folds "mint the bootstrap" into Authorize and
   never has a server-side authentication call at all; UCS gets a dedicated leg for the bootstrap
   and keeps the lookup for `Authenticate`. Nothing hyperswitch does is lost.

### 6.2 A server-initiated Braintree 3DS lookup **does** exist — and it belongs to `Authenticate`

This corrects a dead-end hypothesis that has been floated more than once on this connector.

- `grep -rn "performThreeDSecureLookup\|ThreeDSecureLookup" --include=*.rs
  /home/infamous/hyperswitch1/crates/` returns **no matches** — hyperswitch genuinely never makes
  a server-side lookup. That is a fact about hyperswitch, **not** about the Braintree API.
- The Braintree GraphQL API does expose one. Raw SDL line **6481**
  (`https://raw.githubusercontent.com/braintree/graphql-api/master/schema.graphql`, local copy
  re-read for this run):

  ```graphql
  """
  Attempt to perform 3D Secure Authentication on credit card payment method.
  This may consume the payment method and return a new single-use payment method.
  """
  performThreeDSecureLookup(input: PerformThreeDSecureLookupInput!): PerformThreeDSecureLookupPayload
  ```

  Braintree's step-by-step page frames it as *"An alternative way of performing 3DS … to make the
  3DS call from the server instead of from the client machine."*
- It is **not** this leg. Its device data is mandatory and arrives either as
  `PerformThreeDSecureLookupInput.dfReferenceId` — *"Reference ID used by our MPI provider
  CardinalCommerce to connect the lookup request to the device data that was previously
  collected"*, produced client-side by `threeDSecure.prepareLookup` — or as
  `transactionInformation.browserInformation`, which only exists once a browser has been reached.
  Something must run in the cardholder's browser first, and enabling that is exactly what
  `PreAuthenticate` is for. Its payload (`PerformThreeDSecureLookupPayload`, SDL 8866-8880) is
  field-for-field the UCS `AuthenticateResponse`: a challenge (`acsUrl`/`pareq`/`md`/`termUrl`)
  **or** a frictionless result (`cavv`/`eciFlag`/`liabilityShifted`).
- Its docstring also carries the hand-off the `Authenticate` run must not lose: the lookup *"may
  consume the payment method and return a new single-use payment method"* — the nonce returned by
  `Authenticate`, not the one sent into it, is what the later charge must use.

**`threeDSecurePassThru` is a third, separate ingress and is unchanged by this run.** It is the
external / merchant-MPI path, already implemented on **Authorize** in UCS
(`braintree/transformers.rs`, `convert_external_three_ds_data` + `ThreeDSecurePassThroughInput`,
tests in the same file). The two paths are mutually exclusive by the same predicate in both trees:
`authentication_data` present ⇒ external MPI ⇒ pass-through on Authorize;
`authentication_data` absent + `is_three_ds()` ⇒ Braintree-hosted ⇒ the three authentication legs.
hyperswitch spells the predicate at `hyperswitch1/.../braintree/transformers.rs:416-417`; UCS
spells it in the Braintree-hosted-3DS guard in the Authorize builder, which this run deliberately
**leaves in place** — it is correct until all three legs exist, and retiring it is the
`PostAuthenticate` run's job.

### 6.3 The §P.6.1 decision — option (2), with sandbox evidence

The spec left one implementation decision open: which `RedirectForm` to return.

- **Option (1), the spec's stated preference:** a new `RedirectForm::BraintreeThreeDsSetup`
  variant with `Option` members, requiring changes to
  `crates/types-traits/domain_types/src/router_response_types.rs`, a new `proto/` message, and a
  new `types.rs` arm.
- **Option (2):** issue one HTTP call carrying `tokenizeCreditCard` **and** `createClientToken` as
  two root mutation fields, which yields all four non-`Option` members of the **existing**
  `RedirectForm::Braintree`. The spec required this to be gated on a sandbox call before being
  built on, because multiple root mutation fields are legal GraphQL but undocumented by Braintree.

**Gate run first, before any code was written. It passed.**

```bash
curl -s https://payments.sandbox.braintree-api.com/graphql \
  -H "Authorization: Basic <base64(public_key:private_key)>" \
  -H 'Braintree-Version: 2019-01-01' -H 'Content-Type: application/json' \
  -d '{"query":"mutation braintreeThreeDsPreAuthenticate($card: TokenizeCreditCardInput!, $clientToken: CreateClientTokenInput!) { tokenizeCreditCard(input: $card) { paymentMethod { id } } createClientToken(input: $clientToken) { clientToken } }",
       "variables":{"card":{"creditCard":{"number":"4111111111111111","expirationMonth":"03","expirationYear":"2030","cvv":"123"}},
                    "clientToken":{"clientToken":{"merchantAccountId":"juspay"}}}}'
```

HTTP **200**, no `errors[]`, both root fields resolved:

```json
{
  "data": {
    "tokenizeCreditCard": { "paymentMethod": { "id": "tokencc_bh_9pw8hh_rndmr9_wqtsp8_tqttzp_m26" } },
    "createClientToken": { "clientToken": "eyJ2ZXJzaW9uIjoy…" }
  },
  "extensions": { "requestId": "6044eb7a-e588-46d6-be41-a48536f252e6" }
}
```

Two further facts fell out of the same response and settle open items from §P.13:

- The returned client token base64-decodes to a payload containing
  `"threeDSecureEnabled":true` and `"merchantAccountId":"juspay"` — so a token minted **without**
  `ClientTokenInput.domains` is 3DS-enabled. §P.13 listed that as *"First thing to check in
  sandbox"*; it is now checked.
- `createClientToken` accepts an arbitrary `merchantAccountId` and simply embeds it (a request
  carrying `no_such_account` returned 200 with that value inside the token). So an unknown
  merchant account is **not** an error signal on this leg — do not build a validation on it.

**Decision: option (2) is implemented.** It needs no new domain type and no new proto message.

**One caveat the spec did not anticipate, found during the gRPC test and recorded here so the
"no `types.rs` change" claim is not overstated:** the `PreAuthenticate` response mapper in
`crates/types-traits/domain_types/src/types.rs` does **not** delegate to the shared
`ForeignTryFrom<RedirectForm>` impl — it carries its own inline `RedirectForm` match with a
narrower arm set, and it had no `Braintree` arm, so the first end-to-end call failed with
`UNEXPECTED_RESPONSE_ERROR / "Invalid response type received from connector"` *after* a
successful 200 from Braintree. The fix is one match arm reusing the **existing**
`grpc_api_types::payments::BraintreeData` message and mirroring the shared mapper's Braintree arm.
The two matches are deliberately different (`CybersourceAuthSetup` is mapped in the
`PreAuthenticate` one and rejected in the shared one), so they were not merged. Net blast radius
of option (2) is therefore *one existing-message match arm* versus option (1)'s *new enum variant
+ new proto message + new match arm + regenerated SDK bindings*.

Shipped shape:

| `PreAuthenticateResponse` field | Value | Rationale |
|---|---|---|
| `resource_id` | `None` | A client token is a credential, not a Braintree object — `node(id:)` cannot resolve it, so any id here would be unsyncable. Cybersource sets `None` too. |
| `authentication_data` | `None` | Nothing has been authenticated yet. Populating it would be a fabricated claim about the outcome. |
| `redirection_data` | `RedirectForm::Braintree { client_token, card_token, bin, acs_url }` | The three things `threeDSecure.prepareLookup({nonce, bin})` needs, plus the caller's completion URL. |
| `connector_response_reference_id` | `None` | The payload returns no reference; `clientMutationId` is not selected. Not synthesised. |
| `status_code` | `item.http_code` (always 200) | |
| `resource_common_data.status` | `AttemptStatus::DeviceDataCollectionPending` | Fixed, not mapped — `createClientToken` returns no status of any kind, and DDC is literally what must happen next. Precedent: `worldpay`, `worldpayxml`. |

**Deliberate narrowing vs §P.5.** §P.5 said to accept both `PaymentMethodData::Card` and
`PaymentMethodData::PaymentMethodToken`. Option (2) cannot honour the second: `RedirectForm::
Braintree::bin` is a non-`Option<String>`, `PaymentMethodData::PaymentMethodToken` is
`{ token, token_payment_method_type }` and carries no BIN, and emitting `bin: ""` would be a lie
the browser cannot recover from. The token case is therefore rejected with a populated
`IntegrationError::NotSupported` naming the reason and the remedy, rather than silently degraded.
This is the only place the implementation narrows the spec; option (1)'s `Option<bin>` is what
would lift it, and that trade was made knowingly.

`Braintree-Version` is unchanged at `2019-01-01` (§P.9): every symbol this leg uses predates the
pin and is already in production on the `ClientAuthenticationToken` flow. The `CountryCode`
alpha-3 ceiling at 2021-02-01 is untouched.

### 6.4 Error handling on this leg

Braintree answers HTTP **200** for everything, so success and failure are separated by body shape.
`BraintreePreAuthenticateResponse` is an untagged enum with **`ErrorResponse` listed first** — on a
GraphQL partial success the body carries both a populated `errors[]` and a `data` object, and per
§P.8 the errors must win; `ErrorResponse` requires an `errors` member, so a clean success can
never match it. Verified live (invalid expiry month, HTTP 200):

```json
{ "status": "PENDING", "statusCode": 200,
  "error": { "connectorDetails": { "code": "81712",
                                   "message": "Expiration month is invalid",
                                   "reason": "Expiration month is invalid" } } }
```

`status` stays non-terminal on the error path. That is deliberate and spec-cited (§P.7.1: *"HTTP
200 with a populated `errors[]` → no status write"*; §P.8: *"do not hardcode `Failure` here — this
builder is shared by every Braintree flow"*). The caller learns the failure from the populated
`error` object; there is no sync loop on an authentication-setup leg for a non-terminal status to
strand.

### 6.5 Appendix — reviewer-checklist audit (`grace/braintree_review_checklist.md`)

| Item | Applicable? | How it was satisfied |
|---|---|---|
| 1 — Currency is `common_enums::Currency` | No | `ClientTokenInput` has no currency member; §P.5 forbids validating currency on this leg because no money moves. |
| 2 — amounts use an amount type | No | No amount is sent. The lookup's `amount: Amount!` belongs to `Authenticate`. |
| **3 — PII / credentials are `Secret<…>`** | **Yes** | The client token **is** a credential. `ClientToken.client_token` and `TokenizePaymentMethodData.id` are `Secret<String>`, and `ClientTokenInput.merchant_account_id` is `Secret<String>`. Proven by the live log line: the connector-response event printed `"clientToken":"*** alloc::string::String ***"` and `"id":"*** alloc::string::String ***"`. It is exposed only where it must be — inside the caller-facing `RedirectForm`. |
| 4 — fixed-value strings become enums | No | No fixed-value string field on this leg. |
| **5 — no hardcoded `Failure` in `build_error_response`** | **Yes** | The shared `build_error_response` / `get_error_response` were not touched; `attempt_status` stays `None`. The new error arm writes no status either. |
| **6 — unknown status → `Unspecified`** | Partly | `createClientToken` returns no status enum, so there is nothing to map. The pre-existing `#[serde(other)] Unknown → AttemptStatus::Unspecified` on `BraintreePaymentStatus` (added in the Authorize run) is untouched. The 29-value `ThreeDSecureAuthenticationStatus` table with its mandatory `_ => Unspecified` row is specified in §P.7.2 for the `Authenticate` run and was **not** pre-implemented here (scope discipline). |
| 7 — terminal connector state → terminal UCS state | No | No terminal connector state exists on this leg. |
| 8 — do not map a state the pipeline cannot advance | **Yes** | `DeviceDataCollectionPending` is advanceable *by the caller's browser*, which is exactly what the returned `RedirectForm` enables. The leg is not stranded. |
| 9 — partial capture | No | No capture on this leg. |
| **10 — a 200 carrying a failure body becomes an `ErrorResponse`** | **Yes — the big one for Braintree** | `ErrorResponse` first in the untagged enum so partial successes resolve to the error; live-proven above with `failure_code 81712` / `failure_message "Expiration month is invalid"`. Unit test `pre_authenticate_errors_win_over_a_partially_populated_data_object`. |
| 11 — refund error paths set `attempt_status` | No | Not a refund flow. |
| 12 — Authorize and PSync return the same resource id | No | This leg returns `resource_id: None` by design (§6.3); it creates no syncable resource, so there is no id to disagree about. |
| 13 — idempotency key from `get_merchant_request_id()` | No | `createClientToken` and `tokenizeCreditCard` take no idempotency key; neither moves money. The retry cost is a second unused nonce, recorded in §P.6.1 and accepted. |
| **14 — prefer the per-request field over the connector-config copy** | **Yes** | `PaymentsPreAuthenticateData` has no dedicated `merchant_account_id` member, so the per-request route is the generic `metadata` bag. `resolve_merchant_account_id` reads `request.metadata` **first** (via the existing `extract_metadata_string_field`) and only then falls back to `BraintreeAuthType`. Live-proven: a request with `metadata = {"merchant_account_id":"no_such_account"}` and a config carrying `juspay` produced a client token embedding `"merchantAccountId":"no_such_account"`. The PR2187 reference implementation in the Authorize builder was **not** modified. |
| **15 / 16 — reuse existing helpers; generic logic in `utils.rs`** | **Yes** | Reused `CreditCardData`, `InputData`, `InputClientTokenData`, `ClientTokenInput`, `TokenizeCreditCardData`, `ClientToken`, `ErrorResponse`, `build_error_response`, `get_card_isin_from_payment_method_data`, `extract_metadata_string_field`, `utils::unexpected_response_fail`. `get_braintree_redirect_form` was **refactored to take `Secret<String>` instead of a whole `ClientTokenResponse`** so this leg could share it rather than clone it — its two existing call sites were updated. No new generic helper was added, so nothing belonged in `utils.rs`. |
| 17 — no `billing_full_name` fallback for cardholder name | **Yes** | `cardholder_name: card_data.card_holder_name.clone()` — the same explicit field the Authorize/tokenize builders use after the Authorize run's fix. No billing fallback, omitted when absent. |
| **18 — populated `IntegrationErrorContext`** | **Yes** | Every new error construction carries `additional_context` + `suggested_action` (+ `doc_url` where one exists): the `merchant_account_id` resolution failure, the missing-`payment_method_data` error, and both unsupported-payment-method arms. On the response side, `utils::unexpected_response_fail(http_code, detail)` populates `ResponseTransformationErrorContext` with both the status code and a detail string. No `::default()` anywhere in the new code. |
| 19 — comment non-obvious logic | **Yes** | Comments on: why the document has two root fields, why `ErrorResponse` is first in the untagged enum, why `acs_url` is not an ACS URL, why the status is fixed rather than mapped, why `resource_id`/`authentication_data` are `None`, why a tokenized payment method is rejected, and why the `types.rs` match was not merged with the shared one. |
| **20 — novel local logic needs a `#[cfg(test)]` test** | **Yes — this was blocking on two prior PRs** | Four new tests in the existing module, in its style: `pre_authenticate_sends_one_document_with_two_root_mutations` (document text + `variables` keys match the declared variable names — the failure mode that would break the whole leg), `pre_authenticate_reads_both_root_fields_off_a_success_body`, `pre_authenticate_errors_win_over_a_partially_populated_data_object`, `pre_authenticate_redirect_form_carries_the_ddc_bootstrap_triple` (BIN = first 6 PAN digits, `acs_url` = the return URL). All 22 tests in the module pass. The sandbox and gRPC runs are evidence, not the test coverage. |
| **21 — never guess a production hostname** | **Yes** | No config file was touched. `config/{development,sandbox,production}.toml` already carry `braintree.base_url`, and the flow resolves it through the existing `connector_base_url_payments`. There is no per-flow path on Braintree's GraphQL API. |
| **22 — no unrelated regenerated files** | **Yes** | `scripts/validation/pre-push.sh` regenerated `data/field_probe/braintree.json`, `docs-generated/**` and `examples/braintree/*`; all were reverted, since CI's `auto-fix` job owns them. `git status --porcelain` shows exactly four modified files. |

#### Residual risks recorded by this audit

- **The `Authenticate` gate is still unrun.** §P.9 requires an introspection check that
  `performThreeDSecureLookup` and the 2022/2023-era `ThreeDSecureAuthenticationStatus` values are
  exposed under `Braintree-Version: 2019-01-01` before that leg writes code. This run did not run
  it — it is out of scope and the answer would be stale by then anyway. It remains the first thing
  the `Authenticate` run must do.
- **The end-to-end 3DS loop is not closed.** This leg is provably correct in isolation (live 200,
  a real nonce, a real 3DS-enabled client token, a real BIN), but nothing yet consumes the
  `dfReferenceId` the browser produces from it. Until `Authenticate` lands, a caller that runs
  `PreAuthenticate` has a bootstrap it cannot spend. The Braintree-hosted-3DS guard in the
  Authorize builder still fails such a payment closed, which is the correct interim behaviour.
- **`extensions.errorClass` is still not captured.** `AdditionalErrorDetails` models only
  `legacyCode`, so a retryable `SERVICE_AVAILABILITY` is indistinguishable from a terminal
  `VALIDATION` on every Braintree flow, this one included. Adding it is a shared-struct change
  across all flows and was left out of a single-flow diff, as §P.8 recommends.
- **`PaymentMethodData::PaymentMethodToken` is rejected on this leg** (§6.3). A caller that has
  already tokenized elsewhere must re-send the raw card to PreAuthenticate. Lifting this needs
  option (1)'s `Option<bin>`.

---

## 7. 3DS — leg 2: `Authenticate`

Scope of this section: the **`Authenticate`** leg only (Braintree-hosted 3D Secure, leg 2 —
the server-initiated 3DS lookup). `PostAuthenticate` remains a `not_implemented` stub
(`crates/integrations/connector-integration/src/connectors/braintree.rs`,
`macros::macro_connector_flow_status_impls!`) and is a separate run on this branch; this run
deliberately left it, and the Braintree-hosted-3DS guard in the Authorize builder, untouched.

Section 6 is the handover this section builds on. Read it first.

### 7.1 The `Braintree-Version` gate — §P.9's mandatory precondition — was run, and PASSED

§6.5 recorded *"The `Authenticate` gate is still unrun … it remains the first thing the
`Authenticate` run must do."* It was the first thing this run did, before a line of code was
written, live against `https://payments.sandbox.braintree-api.com/graphql` with
`Braintree-Version: 2019-01-01` — the pinned `BRAINTREE_VERSION_VALUE` (`braintree.rs:69`).

| Gate question (§P.9) | Answer at the pinned version |
|---|---|
| Does `performThreeDSecureLookup` resolve on the root `Mutation` type? | **Yes.** 112 mutation fields; it is one of them. |
| Are the 2022-09-30 input members exposed? | **Yes** — `dataOnlyRequested`, `cardAdd` both present. `merchantInitiatedRequest` (2024-06-13) too. |
| Are the 2022/2023-era `ThreeDSecureAuthenticationStatus` values exposed? | **Yes** — `DATA_ONLY_SUCCESSFUL`, `UNSUPPORTED_ACCOUNT_TYPE`, `LOOKUP_CARD_ERROR`, `LOOKUP_SERVER_ERROR`, `EXEMPTION_LOW_VALUE_SUCCESSFUL`, `EXEMPTION_TRA_SUCCESSFUL`, `MPI_SERVER_ERROR`, `SKIPPED_DUE_TO_RULE` all present. |
| Is `ThreeDSecureLookupData.transactionId` (2022-09-30) exposed? | **Yes.** |

**Consequence: the pin stays at `2019-01-01`.** Option (1) of §P.9's three options. No global
bump, no per-request `Braintree-Version` override on the 3DS legs, no flow-local `get_headers`.

**There is therefore no version tension to report.** The hard ceiling §P.9 warned about — that
raising the pin past `2021-02-01` flips `CountryCode` from alpha-3 to alpha-2 and breaks
`billing_address_uses_alpha3_country_codes_for_the_pinned_braintree_version`
(`braintree/transformers.rs:4790`) — was never approached. That test still passes; it is one of
the 34 green tests in the module after this run. The ceiling is untouched **twice over**, because
the lookup's own billing address takes `ThreeDSecureLookupBillingAddressInput.countryCode` as a
plain `String`, not the versioned `CountryCode` scalar, so the alpha-3/alpha-2 boundary does not
even apply on this leg. There is a regression test for that non-conversion
(`authenticate_billing_country_is_not_converted_to_alpha3`), precisely so a later reader does not
"fix" it into consistency with the Authorize builder.

#### 7.1.1 The gate also refined §P.9's model of what `Braintree-Version` gates

§P.9 inferred from repo evidence that *"the `Braintree-Version` date gates breaking changes;
additive changes are visible to older pins"* and flagged the inference as undocumented. The
introspection shows that model is **too generous in one direction and correct in another**, and
the correction is load-bearing rather than academic:

- §P.9 named the 2020-10-07 retype of `CreditCardDetails.threeDSecure` from
  `ThreeDSecureAuthentication` to `ThreeDSecureDetails` as *"the one genuinely breaking change in
  the cluster"* — the implication being that a 2019-01-01 pin would still see the **old** flat
  shape. It does not. At `Braintree-Version: 2019-01-01` the served schema types that field
  `ThreeDSecureDetails`, i.e. the **new** shape, with the authentication scalars one level down
  under `.authentication`.
- Meanwhile the `CountryCode` alpha-3/alpha-2 change genuinely *is* gated by the pin.

So the operative rule is narrower than §P.9 states: **`Braintree-Version` gates the
interpretation of values, not the shape of the schema.** A selection set written from §P.9's
model — selecting `threeDSecure { cavv eciFlag … }` flat, on the theory that an old pin sees the
old type — would have been rejected outright by the server as a validation error. The shipped
document nests through `threeDSecure { authentication { … } }`, and there is a test asserting the
nesting (`authenticate_document_and_variables_key_agree`).

### 7.2 Topology: UCS runs the lookup on the server; hyperswitch runs it in the browser

This is the deliberate divergence the operator asked to have documented. Every hyperswitch line
below was re-read in `/home/infamous/hyperswitch1` for this run.

**hyperswitch has no server-side Braintree 3DS lookup, anywhere.** A tree-wide grep for
`performThreeDSecureLookup|ThreeDSecureLookup|threeDSecureLookupData|dfReferenceId|prepareLookup`
over `--include=*.rs crates/` returns exactly one hit, and it belongs to a different connector
(`worldpayxml/transformers.rs:596`, `#[serde(rename = "@dfReferenceId")]`). §6.2 asserted this;
it is re-confirmed here rather than inherited.

The lookup instead runs in the cardholder's browser, inside a maud page hyperswitch generates:

| Step | hyperswitch | UCS after this run |
|---|---|---|
| Load the 3DS SDK | `crates/router/src/services/api.rs:1283` — `<script src="https://js.braintreegateway.com/web/3.97.1/js/three-d-secure.js">` | n/a — no SDK is involved server-side |
| Create the 3DS client | `services/api.rs:1310` — `braintree.threeDSecure.create({ authorization: clientToken, version: 2 }, …)` | n/a |
| **Perform the lookup** | `services/api.rs:1317` — `threeDs.verifyCard({ amount, nonce: card_token, bin, addFrame, removeFrame, … })`. **Client-side.** | `mutation braintreeThreeDSecureLookup($input: PerformThreeDSecureLookupInput!)` — `constants::AUTHENTICATE_MUTATION`, `braintree/transformers.rs:72`. **Server-side.** |
| Observe the lookup result | `services/api.rs:1331` — `onLookupComplete: function(data, next) { next(); }` — **the result is bound to `data` and discarded**; the body is a bare `next()` | `PerformThreeDSecureLookupPayload` is parsed in full and mapped onto `AuthenticateResponse` |
| Render the challenge | The Braintree SDK injects the ACS iframe itself (`addFrame` / `removeFrame`) | `RedirectForm::Form { endpoint: acsUrl, method: Post, form_fields: { PaReq, MD, TermUrl } }` |
| Return the outcome to the server | The SDK payload is POSTed as form field `authentication_response` to `{acs_url}` — which is hyperswitch's own `complete_authorize_url`, not an issuer URL (`services/api.rs:1345+`) | The outcome is already on the server; nothing is posted back on this leg |
| Consume it | `CompleteAuthorize` (`braintree.rs:1237`), parsing `BraintreeThreeDsResponse` | `PostAuthenticate` then `Authorize` — out of scope for this run |

**What the divergence actually buys, stated precisely.** hyperswitch's server learns exactly
three facts about a Braintree-hosted 3DS authentication, ever
(`hyperswitch1/.../braintree/transformers.rs:2494-2498`):

```rust
pub struct BraintreeThreeDsResponse {
    pub nonce: Secret<String>,
    pub liability_shifted: bool,
    pub liability_shift_possible: bool,
}
```

parsed at `:2661-2666` out of `BraintreeRedirectionResponse { authentication_response: String }`
(`:2508-2510`). That is the whole of it. hyperswitch never sees `cavv`, `eciFlag`,
`authenticationStatus`, `version`, `directoryServerTransactionId`,
`threeDSecureServerTransactionId`, `acsTransactionId`, `paresStatus`, `transactionStatus`,
`transactionStatusReason` or `cardEnrolled` for a Braintree-hosted 3DS payment — because the only
place those values ever existed was inside the `data` argument that `onLookupComplete` throws away.

UCS's `Authenticate` leg captures all of them server-side, which is what makes
`router_request_types::AuthenticationData` (`router_request_types.rs:136-155`) fillable at all.
Live proof from this run's frictionless case:

```json
"authenticationData": {
  "eci": "05", "cavv": "AJkBBkhgQQAAAE4gSEJydQAAAAA=",
  "threedsServerTransactionId": "852d5a4b-dd50-4d52-83ab-c22300656ad9",
  "messageVersion": "2.1.0", "dsTransactionId": "cd6b7d6d-a33e-4b91-95a0-6ad139e5e820",
  "transStatus": "TRANSACTION_STATUS_SUCCESS",
  "acsTransactionId": "f8c9b0fc-43db-4a08-9063-eccf9cdfafc6",
  "connectorTransactionId": "WQzzmUntlSgHXxLAS0w0" }
```

**The cost, stated just as plainly.** The Braintree JS SDK was doing real work for hyperswitch:
device-data sequencing, ACS iframe lifecycle, and the challenge round-trip. Taking the lookup
server-side means UCS must own each of those explicitly — device data collection became the
`PreAuthenticate` leg (§6), challenge rendering became this leg's `RedirectForm::Form`, and
settling the challenge outcome will be `PostAuthenticate`. This is not a port of anything; there
was nothing to port. It is a deliberately different, finer-grained topology, and §6.1's
conclusion — *"UCS's split is strictly finer … nothing hyperswitch does is lost"* — now holds
with one leg still outstanding.

**`threeDSecurePassThru` remains a third, separate ingress and is unchanged.** As in §6.2:
`authentication_data` present ⇒ external MPI ⇒ pass-through on Authorize;
`authentication_data` absent + `is_three_ds()` ⇒ Braintree-hosted ⇒ these three legs. This leg
**rejects** a request that arrives carrying `authentication_data`, with a populated error rather
than silently cross-wiring it into the lookup — the two topologies must not merge.

### 7.3 Three live findings that contradicted the inherited design, and what each changed

The gate call was cheap, so the run also exercised the real mutation end to end against four 3DS
test cards before designing. Three results contradicted §P.12.1 / §6 and changed the shipped code.

#### 7.3.1 `dfReferenceId` is **not** mandatory

§P.12.1 and §6.2 both stated that the lookup's device data is required and arrives *"either as
`PerformThreeDSecureLookupInput.dfReferenceId` … or as `transactionInformation.browserInformation`"*,
and §6.2 concluded from it that *"Something must run in the cardholder's browser first, and
enabling that is exactly what `PreAuthenticate` is for."*

The second half of that is weaker than it reads. All four live lookups in this run omitted
`dfReferenceId` entirely and completed normally on `transactionInformation.browserInformation`
alone — including the one that produced a genuine `CHALLENGE_REQUIRED` with a real ACS URL.

What shipped implements **both** ingresses: `dfReferenceId` is preferred when the caller returns
one in `PaymentsAuthenticateData.redirect_response` (read under either spelling — test
`authenticate_df_reference_id_is_read_under_either_spelling`), and `browserInformation` built from
`PaymentsAuthenticateData.browser_info` is the fallback. An error is raised only when neither is
available.

The practical effect on §6's story: `PreAuthenticate` is still the right leg and still improves
the authentication outcome, but `Authenticate` is **not hard-blocked** on it. §6.5's residual risk
*"a caller that runs `PreAuthenticate` has a bootstrap it cannot spend"* is now half-resolved —
the bootstrap is spendable, though the loop still does not close until `PostAuthenticate` lands.

#### 7.3.2 `threeDSecureLookupData` is always present — the discriminator trap

This is the one that would have shipped a real bug. The natural reading of
`PerformThreeDSecureLookupPayload` is that `threeDSecureLookupData` is the *challenge* payload and
`paymentMethod…authentication` is the *frictionless* payload, so its presence discriminates.

It does not. `threeDSecureLookupData` came back **non-null on all four outcomes**, frictionless
success included. On the non-challenge outcomes `acsUrl` and `pareq` are `null` while
`authenticationId`, `md`, `termUrl`, `transactionId` and `version` are still populated. A
`threeDSecureLookupData.is_some()` discriminator would have emitted a `RedirectForm` on every
successful frictionless authentication and sent the cardholder to a challenge that does not exist.

Shipped discriminator, in `is_braintree_challenge`:
`authenticationStatus == CHALLENGE_REQUIRED` **and** `acsUrl.is_some()` — both halves, belt and
braces. Commented at the call site with the wrong-form rationale, and pinned by
`authenticate_discriminator_survives_the_always_present_lookup_data_trap`, which feeds it a
frictionless body carrying a fully populated `threeDSecureLookupData` and asserts no form is
produced. Live-confirmed: the frictionless gRPC response in §7.4 carries no `redirectionData`.

#### 7.3.3 The payment method id always changes

§P.12.1 carried the docstring's hedge — the lookup *"may* consume the payment method and return a
new single-use payment method". In practice it is unconditional: all four calls returned a
`paymentMethod.id` different from the `paymentMethodId` sent in, and the returned id is a bare
UUID rather than a `tokencc_`-prefixed nonce. Replaying a spent nonce returns
`"Nonce is already consumed"`.

So `resource_id` is the **new** id, commented as such, and the later charge must spend that one.
Live: sent `tokencc_bh_3sdwx2_463jj4_nzsv5m_nhfsvn_hwz`, got back
`84294a9b-8d1d-14ed-7e25-c1cab84f06ed`.

### 7.4 What shipped

| `AuthenticateResponse` field | Value | Rationale |
|---|---|---|
| `resource_id` | the **new** `paymentMethod.id` | §7.3.3 — the input nonce is spent and cannot be charged. |
| `redirection_data` | `RedirectForm::Form { endpoint: acsUrl, method: Post, form_fields: { PaReq, MD, TermUrl } }`, **only** on a challenge | §7.3.2. The classic 3DS browser POST. Chosen because the `Authenticate` response mapper (`types.rs:19798-19930`) already supports `Form` — see the blast-radius note below. |
| `authentication_data` | the full `ThreeDSecureAuthentication` object mapped onto `AuthenticationData`, on **both** branches | The challenge branch has an ECI and DS/ACS/3DS-server transaction ids even before the challenge runs; dropping them would discard the only server-side record of the authentication. |
| `connector_feature_data` | `braintree_three_ds` blob carrying `liability_shifted`, `liability_shift_possible`, `card_enrolled`, `authentication_status`, `pares_status`, `transaction_status_reason`, `x_id`, `bin`, `last4`, `brand_code`, `authentication_id`, `lookup_transaction_id` | §P.7.2 is explicit that `liabilityShifted` must be **read, not inferred** from the status (`DATA_ONLY_SUCCESSFUL` authenticates without shifting liability). `AuthenticationData` has no member for it, so it rides here rather than being lost. |
| `connector_response_reference_id` | `authenticationId` | The one stable id Braintree returns for the authentication itself. |
| `resource_common_data.status` | mapped from `authenticationStatus` via the §P.7.2 table | Not fixed, unlike `PreAuthenticate` — this leg genuinely has a status to map. |

**Blast radius: one 8-line and one 7-line addition, in one struct and its one construction site.**
No new `RedirectForm` variant, no `router_response_types.rs` change, no new proto message, no
regenerated SDK bindings, no `config/*.toml` change. The contrast with §6.3's option-(1)/option-(2)
trade is deliberate: choosing `RedirectForm::Form` over a bespoke `RedirectForm::BraintreeThreeDs`
variant is the same "reuse the existing shape" call, and it paid off more cleanly here because the
`Authenticate` mapper already handles `Form` (it rejects `RedirectForm::Braintree` at its `_ =>`
arm, `types.rs:19908` — worth knowing, since §6.3's `PreAuthenticate` leg relies on exactly that
variant on the *other* mapper).

The one cross-crate change is `PaymentsAuthenticateData::metadata`
(`domain_types/src/connector_types.rs`, 8 lines) plus its population in `types.rs` (7 lines),
copied verbatim from the identical `PaymentsPreAuthenticateData` precedent. It exists for
checklist item 14: the gRPC request has always carried
`PaymentMethodAuthenticationServiceAuthenticateRequest.metadata` (field 7), but `types.rs` read it
only to pull out `sdk_information` / `device_channel` and then dropped the bag — so a connector
whose Authenticate leg needs a merchant-scoped identifier could not reach it. With it, this leg
reuses `resolve_merchant_account_id` (`braintree/transformers.rs:4444`) **verbatim** from the
PreAuthenticate leg rather than cloning or generalising it. Live-proven: a request carrying
`metadata = {"merchant_account_id":"not_a_real_merchant_account"}` against a config saying
`juspay` logged `BRAINTREE: Picking merchant_account_id from the per-request metadata`.

**Status table.** §P.7.2's table is implemented, with one correction forced by the gate: it
describes **29** values from the master SDL, four of them `@deprecated`. The live schema serves
**25** — the four deprecated ones (`AUTHENTICATE_SIGNATURE_VERIFICATION_FAILED`,
`AUTHENTICATE_SUCCESSFUL_ISSUER_NOT_PARTICIPATING`, `AUTHENTICATION_BYPASSED`, `LOOKUP_ENROLLED`)
have been **removed**, not merely deprecated. Their serde arms are kept as §P.7.2 asks
(deserialize-only, harmless) but are unreachable; the real protection is the mandatory
`#[serde(other)] Unknown → AttemptStatus::Unspecified` catch-all.

**Live outcomes, all four exercised over gRPC:**

| Card | `authenticationStatus` | UCS status | `redirectionData` | `cavv` |
|---|---|---|---|---|
| `4000000000001091` | `CHALLENGE_REQUIRED` | `AUTHENTICATION_PENDING` (non-terminal) | `form` → `0merchantacsstag.cardinalcommerce.com` | absent |
| `4000000000001000` | `AUTHENTICATE_SUCCESSFUL` | `AUTHENTICATION_SUCCESSFUL` | **none** | present, ECI 05 |
| `4000000000001018` | `AUTHENTICATE_FRICTIONLESS_FAILED` | `AUTHENTICATION_FAILED` (terminal) | none | absent |
| replayed nonce | — (`errors[]`) | `PENDING`, no terminal write | none | — |

### 7.5 Error handling on this leg

Unchanged in principle from §6.4, and for the same reason: Braintree answers HTTP 200 for
everything. `BraintreeAuthenticateResponse` is an untagged enum with **`ErrorResponse` first**,
and that variant requires an `errors` member, so a clean success can never match it.

What this leg adds to §6.4's evidence is that the ordering is not merely defensive here — it is
**necessary**. Both real error bodies observed carry a `data` key alongside the populated
`errors[]`:

```json
{ "errors": [ { "message": "Payment method nonce not found",
                "path": ["performThreeDSecureLookup"],
                "extensions": { "errorClass": "VALIDATION", "errorType": "user_error" } } ],
  "data": { "performThreeDSecureLookup": null },
  "extensions": { "requestId": "d1bb4b89-…" } }
```

A `data`-key-presence test would classify both as successes. Schema-validation errors, by
contrast, carry `errors[]` and **no** `data` key at all, so the error variant must tolerate its
absence too. Both shapes are covered by
`authenticate_errors_win_over_a_populated_data_object`. The existing
`GenericBraintreeResponse<T>` was deliberately **not** reused here, because it lists success
first — reusing it would have inverted the precedence.

Live, from the replayed-nonce case: `failure_message "Nonce is already consumed"`, and
**no terminal status written** — the response came back `PENDING` from the caller's own fallback.
That is §P.7.1 / §P.8 behaviour and checklist item 5, demonstrated rather than asserted.

One new confirmation for §6.5's standing note: `extensions.errorClass` **is** on the wire
(`"VALIDATION"`, with `errorType: "user_error"`). §P.13 listed it as *"not modelled"*, which is
about the UCS struct rather than availability; it is now known to be available, not merely
suspected. `AdditionalErrorDetails` still models only `legacyCode`. Capturing `errorClass` remains
a shared-struct change across every Braintree flow and stays out of a single-flow diff, as §P.8
recommends — but the "is it even there?" question is now closed.

### 7.6 Appendix — reviewer-checklist audit (`grace/braintree_review_checklist.md`)

| Item | Applicable? | How it was satisfied |
|---|---|---|
| 1 — Currency is `common_enums::Currency` | **Yes** | `request.currency: Option<Currency>` is consumed as the enum for the amount conversion; `None` raises `MissingRequiredField`. Never stringified. |
| **2 — amounts use an amount type** | **Yes** | `PerformThreeDSecureLookupInput.amount: StringMajorUnit`, produced only via `item.connector.amount_converter` — the connector's existing converter (`braintree.rs:467-469`), the same type as every other money field in the file. Test `authenticate_amount_is_major_units_through_the_connector_converter` pins `MinorUnit(1000)/USD → "10.00"`. |
| **3 — PII / credentials are `Secret<…>`** | **Yes** | `payment_method_id`, `merchant_account_id`, `df_reference_id`, `cavv`, the returned `paymentMethod.id`, every billing name/line/postal field, `ipAddress`, and `termUrl` are `Secret<…>`; `email` is `pii::Email`. `termUrl` is `Secret` specifically because it embeds a signed `authorization_fingerprint` JWT — it is exposed only at the point it enters `form_fields`, which is unmasked by construction, and that limitation is commented inline. |
| **4 — fixed-value strings become enums** | **Yes** | `ThreeDSecureAuthenticationStatus`, `ThreeDSecureAuthenticationStatusIndicator`, `ThreeDSecureCardEnrolled`, `ThreeDSecureDeviceChannel`. `eciFlag` is deliberately left `String` — its values are network-scoped and the existing pass-through code already treats it as one. |
| **5 — no hardcoded `Failure` in `build_error_response`** | **Yes** | The shared `ConnectorCommon::build_error_response` was not touched and the new error arm writes no status at all. A second, subtler instance was caught and closed: `common_enums::TransactionStatus`'s `Default` is `Failure`, so `to_trans_status` returns `Option` and maps `Unknown → None` rather than letting an `unwrap_or_default()` manufacture a failed authentication. Asserted in `authenticate_trans_status_maps_eight_to_eight_and_never_defaults`. |
| **6 — unknown status → `Unspecified`** | **Yes** | `#[serde(other)] Unknown` on all three new response enums, mapping to `AttemptStatus::Unspecified`. Test `authenticate_unknown_status_maps_to_unspecified_not_pending_or_failure`. This is also what actually protects against the four removed deprecated values (§7.4). |
| **7 — terminal connector state → terminal UCS state** | **Yes** | All 15 reachable terminal Braintree statuses map to `AuthenticationFailed`, and the test asserts `is_terminal_status()` on every one. |
| **8 — do not map a state the pipeline cannot advance** | **Yes** | `CHALLENGE_REQUIRED → AuthenticationPending` is non-terminal **and** always carries a usable `RedirectForm::Form` — the two are inseparable, which is why the discriminator requires `acsUrl.is_some()` as well as the status (§7.3.2). Verified live. The `AuthenticationSuccessful` rows are advanceable because Braintree's Authorize already exists. |
| 9 — partial capture | No | No capture on an authentication leg. |
| **10 — a 200 carrying a failure body becomes an `ErrorResponse`** | **Yes** | §7.5. `ErrorResponse` first in the untagged enum; `GenericBraintreeResponse<T>` deliberately not reused because it orders success first. Live-proven with populated `code`/`message`/`reason`. |
| 11 — refund error paths set `attempt_status` | No | Not a refund flow. |
| 12 — Authorize and PSync return the same resource id | No | Neither flow was touched. Worth noting for the `PostAuthenticate` run: this leg's `resource_id` is a *payment method* id, not a transaction id, and the two must not be conflated. |
| 13 — idempotency key from `get_merchant_request_id()` | No | `PerformThreeDSecureLookupInput` has no idempotency member — `clientMutationId` is a pure echo. No UUID is minted, so the anti-pattern the item guards against cannot occur. The leg is recorded as non-idempotent; a retry costs one more consumed nonce. |
| **14 — prefer the per-request field over the connector-config copy** | **Yes — and it is the reason for the only cross-crate change** | §7.4. `resolve_merchant_account_id` reused verbatim from the PreAuthenticate leg, reading `request.metadata` first and falling back to `BraintreeAuthType`. The `PaymentsAuthenticateData::metadata` addition exists solely to make that reachable. Live-proven via the server log line. The PR2187 reference implementation in the Authorize builder was not modified. |
| **15 / 16 — reuse existing helpers; generic logic in `utils.rs`** | **Yes** | Reused `resolve_merchant_account_id` (:4444), `extract_metadata_string_field` (through it), `build_error_response` (:1555), `utils::unexpected_response_fail`, the `GenericBraintreeRequest` / `GenericVariableInput` aliases (:87-101) and the existing `ErrorResponse` / `ErrorDetails` / `AdditionalErrorDetails` structs. Nothing was cloned. Every new helper is Braintree-schema-specific (GraphQL input shapes, Braintree enums), so nothing belonged in `utils.rs`. |
| **17 — no `billing_full_name` fallback for cardholder name** | **Yes** | `givenName` / `surname` map the explicit first/last name fields and each is omitted when absent. No billing-name fallback. |
| **18 — populated `IntegrationErrorContext`** | **Yes** | All new error constructions — missing `payment_method_data`, missing `currency`, amount-conversion failure, unsupported payment method, `authentication_data` present (the external-MPI rejection), and missing device data — carry `additional_context` + `suggested_action`, with `doc_url` on the three that have a real Braintree doc. No `::default()` in the new code. |
| **19 — comment non-obvious logic** | **Yes** | Comments on: the §7.3.2 discriminator **including why the obvious form is wrong**, the §7.3.3 new-nonce rule, the §7.1.1 `.authentication` nesting rule, Braintree's `javascriptEnabled` spelling, `ipAddress` sitting on `transactionInformation` rather than inside `browserInformation`, why `countryCode` is not alpha-3-converted here, why `termUrl` is `Secret`, why `ErrorResponse` is first in the untagged enum, why four unreachable status arms are kept, and the `TransactionStatus::default() == Failure` trap. |
| **20 — novel local logic needs a `#[cfg(test)]` test** | **Yes — 12 new tests**, against the PreAuthenticate run's 4 | Document/variables-key agreement including the inline fragment and the `.authentication` nesting; major-unit amount conversion; Braintree browser-info spellings and `ipAddress` placement; alpha-3 non-conversion and no-empty-billing-object; `dfReferenceId` under either spelling; the challenge discriminator **including the always-present-`threeDSecureLookupData` trap** and each half-condition separately; the ACS step-up triple and that a frictionless body cannot produce a form; the terminal / non-terminal / advanceable status split; unknown → `Unspecified`; the 8-to-8 `trans_status` map and its never-default property; the new-nonce and liability-boolean read; errors-win-over-a-populated-`data`. **34 tests in the module, all green** — the 22 that existed after the PreAuthenticate run, plus these 12. The sandbox and gRPC runs are evidence, not the test coverage. |
| **21 — never guess a production hostname** | **Yes** | No config file was touched. The URL resolves through the existing `connector_base_url_payments`; Braintree's GraphQL API has no per-flow path. |
| **22 — no unrelated regenerated files** | **Yes** | `git status --porcelain` shows exactly five changed files plus `data/integration-source-links.json`, which the Links Agent legitimately extended this run (16 → 28 Braintree doc URLs). Nothing under `data/field_probe/`, `docs-generated/` or `examples/` is dirty. |

#### Residual risks recorded by this audit

- **`PostAuthenticate` is still a stub, so the 3DS loop still does not close.** A challenge
  emitted by this leg has nowhere to land: the cardholder can be sent to the ACS, but nothing
  ingests the result and turns it into a charge. The Braintree-hosted-3DS guard in the Authorize
  builder still fails such a payment closed, which remains the correct interim behaviour. That is
  the next run on this branch, and §6.5's residual risk stands, downgraded: the bootstrap is now
  spendable (§7.3.1), the outcome is not yet bankable.
- **The ACS form-field key casing (`PaReq` / `MD` / `TermUrl`) is inferred, not browser-observed.**
  It is the 3DS1-era convention every other connector in this repo uses and Braintree returns the
  fields under exactly those names, but no challenge was driven to completion in a real browser
  this run. If a live challenge ever 400s at the ACS, this is the first thing to check.
- **The leg is not idempotent** (checklist 13). A retried lookup consumes a second nonce. Braintree
  offers no idempotency member on this input; there is nothing to fix, only to know.
- **`extensions.errorClass` is captured on the wire but still not modelled** — see §7.5.
  Unchanged from §6.5 except that availability is now confirmed rather than assumed.
- **Braintree still has no `next_authentication_step` override**, so the composite-authorize
  dispatcher will not route the 3DS trio by itself. Consistent with how the PreAuthenticate run
  left things, and premature to add while `PostAuthenticate` is a stub — it belongs to that run.
- **`strum::Display` vs serde spelling.** The three new status enums carry
  `#[strum(serialize_all = "SCREAMING_SNAKE_CASE")]` so their `Display` matches the raw Braintree
  strings that go into `connector_feature_data`; without it they rendered Rust variant names
  (`"ChallengeRequired"`). The **pre-existing** `BraintreePaymentStatus` has the same latent
  mismatch and was deliberately not touched — out of scope for this diff, but it is a real papercut
  waiting for whoever reads `connector_feature_data` on the Authorize path.

---

## 8. 3DS — leg 3: `PostAuthenticate`

Scope of this section: the **`PostAuthenticate`** leg (Braintree-hosted 3D Secure, leg 3 — the
settlement of the authentication after the cardholder completes the ACS challenge leg 2 emitted).
With it the trio is complete, so this section also owns the two things the prior two runs deferred
to it: **retiring the Braintree-hosted-3DS guard** in the Authorize builder, and **overriding
`next_authentication_step`** so the composite dispatcher routes the trio at all.

Sections 6 and 7 are the handover this section builds on. Read them first. Where §6 or §7 is
corrected below, the correction is stated explicitly rather than left for the reader to infer;
neither section was edited.

### 8.1 The central question, and the answer — with schema evidence

The brief for this run was blunt: *do not fabricate a mutation or a field; if Braintree has no
separate post-challenge retrieval call, say so with schema evidence and implement the honest
minimum.* That was the right instruction, and the honest answer turned out to be neither "there is
a completion mutation" nor "this leg is a local assembly". It is a third thing.

**Braintree offers no post-challenge completion *mutation*, and no authentication-retrieval
*query*. What it offers is that the ACS result is written onto the payment method, and the payment
method is a `Node`.** Live introspection at `Braintree-Version: 2019-01-01`, three independent
ways:

| Probe | Result |
|---|---|
| Root `Mutation` field list (112 fields) grepped case-insensitively for `3d`, `3ds`, `threeDSecure`, `challenge` and the `authentication` stem | **Exactly one match** — `performThreeDSecureLookup`, which is leg 2. There is no completion mutation to call. |
| Root `Query` field list (26 fields, served order) | No `threeDSecureAuthentication`, no `threeDSecureLookup`, no `paymentMethod(id:)`. `node(id: ID!)` is the only id-addressable entry point. |
| `initializeChallengeWithLookupResponse` | **Does not exist as a mutation at all.** It is a browser JS-SDK method. |

That last row corrects the tech spec twice over: both §P.12.2 and §A.12 framed this leg as
*"ingests the browser's `initializeChallengeWithLookupResponse` outcome"*. There is nothing on the
server to ingest it with. The spec's §PA section supersedes both.

So the shipped leg is a **`node(id: <the NEW paymentMethod.id leg 2 returned>)` readback** — one
GraphQL query, one real HTTP POST to the same endpoint with the same headers. Not a mutation, not
a local assembly, not a no-op.

**And it genuinely works, which was proved rather than assumed.** The decisive experiment POSTed a
PaRes to Braintree's own `termUrl` and then re-read the *same* payment-method id:

| Field | Before the ACS post-back | After the ACS post-back |
|---|---|---|
| `authenticationStatus` | `CHALLENGE_REQUIRED` | **`AUTHENTICATE_UNABLE_TO_AUTHENTICATE`** |
| `transactionStatus` | `CHALLENGE_REQUIRED_FOR_AUTHENTICATION` | **`UNABLE_TO_COMPLETE_AUTHENTICATION`** |
| `paresStatus` | `null` | **`UNABLE_TO_COMPLETE_AUTHENTICATION`** |

`node(id:)` is a **live server-side view** of the authentication and it transitions when the ACS
result reaches Braintree. (The probe's PaRes was synthetic, hence
`UNABLE_TO_AUTHENTICATE` rather than `AUTHENTICATE_SUCCESSFUL`; the mechanism is what the
experiment establishes. A real browser challenge driven to a post-challenge CAVV remains
unobserved — recorded in §8.8.)

Three further facts fell out of the same experiments and each changed the shipped code:

1. **`node(id:)` resolves a single-use nonce, raw and verbatim.** This closes the standing unknown
   §P.13 opened and §A.13 carried forward. `base64(id)`, `base64("PaymentMethod:"+id)`, the spent
   `tokencc_…` nonce and `authenticationId` (raw and base64) all return `NOT_FOUND`. There is no
   Relay global-id wrapper.
2. **The readback is non-consuming and idempotent** — two consecutive calls returned byte-identical
   `data`, and it is lossless relative to the lookup payload (all 14 `ThreeDSecureAuthentication`
   members, `cavv` included). This leg is therefore safely re-runnable and pollable, the exact
   opposite of leg 2, which destroys its input (§7.3.3).
3. **The browser post-back never reaches the merchant.** Braintree 302s to
   `assets.braintreegateway.com` with `frame-ancestors 'self'`; in the JS-SDK topology the SDK's
   iframe catches it and `postMessage`s out. In a server-side, no-SDK integration there is no
   receiver. Consequence, and it is a real divergence from most `PostAuthenticate`
   implementations: **`PaymentsPostAuthenticateData.redirect_response` carries nothing this leg
   needs.** `get_redirect_response_payload()` is deliberately not called, `params` is not required,
   and a `None` `redirect_response` is not an error. It is a *timing signal* that the browser is
   back, not a data channel — commented at the call site so a later reader does not "fix" it.

Braintree's own documentation covers none of this: the step-by-step guide documents
`threeDSecure.initializeChallengeWithLookupResponse` as a client-side SDK call and is silent on
where the ACS posts and how a server learns the outcome; `TermUrl`, `PaReq` and `MD` appear in
neither it nor the advanced-options page. Everything above is live-observed, not documented.

### 8.2 Topology: hyperswitch's completion leg is a **charge**; UCS's is a **retrieval**

Every hyperswitch line below was re-read in `/home/infamous/hyperswitch1` for this run.

| Step | hyperswitch | UCS after this run |
|---|---|---|
| Flow marker | `CompleteAuthorize` — `braintree.rs:1237`. **UCS has no such marker** (§6.1) | `PostAuthenticate` |
| What the completion leg *is* | a **charge**: `get_request_body` builds `chargeCreditCard` / `authorizeCreditCard` with `payment_method_id: three_ds_data.nonce` (`braintree/transformers.rs:2651-2716`) | a **retrieval**: `node(id:)`. The charge stays in `Authorize`, where it belongs |
| Where the result comes from | the browser — `BraintreeRedirectionResponse { authentication_response: String }` (`transformers.rs:2508-2510`), parsed at `:2661-2666` | Braintree's server, off the payment method |
| What the server learns | **three fields, ever**: `BraintreeThreeDsResponse { nonce, liability_shifted, liability_shift_possible }` (`transformers.rs:2494-2498`) | all 14 members of `ThreeDSecureAuthentication` |
| What is discarded | everything else — `onLookupComplete: function(data, next) { next(); }` (`router/src/services/api.rs:1331`) binds the lookup result to `data` and throws it away | nothing |
| Browser post-back → server | `ConnectorRedirectResponse::get_flow_type` (`braintree.rs:1191-1233`): `CompleteAuthorize \| PaymentAuthenticateCompleteAuthorize ⇒ Trigger`; a payload arriving on `PaymentAction::PSync` is the failure path and yields `AttemptStatus::AuthenticationFailed` (`:1221`) | no post-back is consumed; the outcome is read from Braintree (§8.1) |

**Two facts worth stating precisely.**

1. **hyperswitch parses two booleans it never reads.** `liability_shifted` and
   `liability_shift_possible` are deserialized at `transformers.rs:2494-2498`, and a tree-wide grep
   over `crates/hyperswitch_connectors/src/connectors/braintree*` finds **no read of either** —
   only the two field declarations. So hyperswitch's *effective* learning from a Braintree-hosted
   3DS authentication is **one field: the nonce.** UCS now carries both booleans *and* surfaces
   them, which is what finally gives §7.4's *"`liabilityShifted` must be read, not inferred"* rule
   somewhere to land. Live, from this run's frictionless case:
   `"liability_shifted":true,"liability_shift_possible":true,"card_enrolled":"YES"`.
2. **hyperswitch's approach is structurally unavailable to UCS.** Its `auth_response` exists only
   because the Braintree **JS SDK** receives the `assets.braintreegateway.com` post-back and hands
   it to merchant JavaScript. UCS's server-side topology has no such receiver — the payload is not
   merely unused, it is unreachable. The `node(id:)` readback is what replaces it, and it returns
   strictly more. This is not a preference; it is the only path that exists.

§7.2's conclusion — *"UCS's split is strictly finer … nothing hyperswitch does is lost"* — now
holds with **no leg outstanding**.

### 8.3 Satisfying the HS driving contract

The operator's contract for this run was verified against hyperswitch rather than assumed:
`post_authentication_step` (`crates/router/src/core/payments/flows/complete_authorize_flow.rs:438-447`)
gates on `is_post_authentication_flow_required(CurrentFlowInfo::CompleteAuthorize { .. })`, and in
the reference connectors (`barclaycard.rs:1679-1694`, `cybersource.rs:2584-2606`) that gate is the
exact **complement** of the `Authenticate` gate (`barclaycard.rs:1661-1671`): `redirect_response.params`
present and non-empty ⇒ `Authenticate`; absent or empty ⇒ `PostAuthenticate`.

UCS expresses the same split through `ValidationTrait::next_authentication_step`
(`types-traits/interfaces/src/connector_types.rs:276`), which the composite dispatcher calls in a
loop (`crates/internal/composite-service/src/payments.rs:819-905`). **Braintree did not override
it** — §7.5 recorded that as a residual risk, and the consequence is stronger than "a missing
nicety": `pattern_postauthenticate.md` is explicit that *a connector that implements this flow but
does not override that method has an unreachable implementation*. All three legs were unreachable
code. This run adds the override, on Cybersource's params/no-params axis:

| `(RedirectState, completed_step)` | Step |
|---|---|
| `(InitialRequest, None)` | `PreAuthenticate` |
| `(RedirectWithParams, None)` | `Authenticate` — the DDC return carries the `dfReferenceId` |
| `(RedirectWithParams, Some(Authenticate))` | `Authorize` — frictionless within the same call |
| `(RedirectWithoutParams, None)` | **`PostAuthenticate`** |
| `(RedirectWithoutParams, Some(PostAuthenticate))` | `Authorize` |
| anything else / not (ThreeDs ∧ Card) | `Authorize` |

The `RedirectWithoutParams` arm is load-bearing and it is not arbitrary: **the ACS posts its PaRes
to Braintree's own `termUrl`, not to UCS** (§8.1), so a parameterless browser return is exactly the
signature of *"the challenge is over, go read the result"*. That is precisely what makes HS's
complement gate the right shape for Braintree too, and it is why Cybersource's shape was chosen
over Getnet's (`getnet.rs:166-200`), which routes the ACS return through
`(RedirectWithParams, Some(Authenticate))` — true for Getnet, false for Braintree, whose
`Authenticate` leg breaks the dispatcher loop the moment it emits a challenge.

One consequence for whoever raises the HS-side PR (`grace/braintree_hs_side_notes.md` has the
full scope): HS's `braintree.rs` still overrides none of the three
`is_*_flow_required` methods, so the trio remains unreachable *from HS* until that PR lands. This
run makes it reachable from UCS's own composite path, which is what the gRPC evidence below
exercises.

### 8.4 What shipped

| `PostAuthenticateResponse` field | Value | Rationale |
|---|---|---|
| `authentication_data` | the full `threeDSecure.authentication` block mapped onto `AuthenticationData` | The one thing the leg exists to produce. Mapped by the **same function** leg 2 uses — `build_three_ds_authentication_data` was extracted and leg 2's inline copy now calls it, so the two cannot drift (checklist 15). |
| `connector_response_reference_id` | the **payment-method id that was read** | The variant has no `resource_id`, and on this leg the id the caller needs next is the spendable one. **Deliberate divergence from §7.4**, which put `authenticationId` here — `node(id:)` does not return an `authenticationId` at all (it lives on `ThreeDSecureLookupData`, which has no query entry point). |
| `status_code` | `item.http_code` (always 200) | |
| `resource_common_data.status` | the **§A.7 / §7.4 table verbatim** — same enum, same `From` impl, keyed on `authenticationStatus` | No second table was invented. |
| `resource_common_data.connector_feature_data` | the `braintree_three_ds` blob, refreshed from the readback, carrying `payment_method_id` + the liability booleans + `card_enrolled` + raw statuses + `bin`/`last4`/`brand_code` | **This is how the verified nonce leaves a variant that has no slot for it** — see below. |

`PaymentsResponseData::PostAuthenticateResponse` (`connector_types.rs:2082-2086`) carries only
`{authentication_data, connector_response_reference_id, status_code}`: no `resource_id`, no
`redirection_data`, no `connector_feature_data`. The nonce problem is solved without touching it,
because `types.rs:20023-20029` sources the gRPC response's `connector_feature_data` from
**`resource_common_data`**, independently of the response variant, and sets it on *both* arms. So
writing `resource_common_data.connector_feature_data` in the response transformer surfaces the blob
with **zero `types.rs` and zero `proto/` change**.

Returning `TransactionResponse` instead — which `types.rs:20054-20080` does accept from this flow —
was rejected: that arm sets `authentication_data: None` unconditionally (`types.rs:20071`), which
would discard the CAVV, ECI, DS-Trans-Id and trans-status, i.e. the entire output of the trio, and
would break the composite dispatcher, which forwards
`post_authenticate_response.authentication_data` into the Authorize request
(`composite-service/src/transformers.rs:292-296`).

**Blast radius outside the connector: zero.** No `types.rs`, no `connector_types.rs`, no
`router_response_types.rs`, no `proto/`, no `config/*.toml`, no regenerated SDK bindings. This is
strictly narrower than leg 1 (one `types.rs` match arm) and leg 2 (`PaymentsAuthenticateData::metadata`,
8 + 7 lines). Three files changed: `braintree.rs`, `braintree/transformers.rs`, and one line in
`connector_specs/braintree/specs.json`.

**`metadata` was not needed, and that is a finding, not a gap.** `node(id: ID!)` takes one
argument; the merchant is scoped by Basic auth. So `PaymentsPostAuthenticateData`'s missing
`metadata` member — which looked like it would force leg 2's cross-crate change again — is a
non-issue, `resolve_merchant_account_id` is deliberately **not** called here, and checklist item 14
is N/A on this leg.

`Braintree-Version` is unchanged at `2019-01-01`. Nothing this leg selects postdates the pin, so
§7.1's gate covers it and no new gate was required. The `CountryCode` alpha-3 ceiling
(`transformers.rs:4790`) is untouched — this leg sends no address at all.

**Two selection-set footguns were captured live and are pinned by a test.** `createdAt` on a
single-use payment method returns a *partial* `NOT_IMPLEMENTED` error that would drag an otherwise
good readback into the error arm; `authenticationInsight` requires an `input` argument. Neither is
selected, and `post_authenticate_document_selects_neither_created_at_nor_authentication_insight`
asserts their literal absence from the document.

### 8.5 Retiring the Authorize guard — and the cross-wiring trap

§6.2 and §A.12 both assigned the Braintree-hosted-3DS guard (`transformers.rs:1011-1045`) to this
run. Its stated reason — *"consuming the nonce that comes back needs a CompleteAuthorize flow,
which this connector does not implement"* — is now obsolete, and the sentence was doubly wrong
anyway: UCS has no `CompleteAuthorize` marker at all (§6.1).

**The trap, which a naive retirement walks straight into.** After this run an Authorize request can
carry `authentication_data` from two mutually exclusive ingresses:

| Ingress | How `authentication_data` gets there | What Authorize must send |
|---|---|---|
| **External MPI** (already shipped) | the caller puts it on the original Authorize request | `options.threeDSecureAuthentication.passThrough` |
| **Braintree-hosted** (legs 1-3) | the composite dispatcher copies PostAuthenticate's into the Authorize request (`composite-service/src/transformers.rs:292-296`); HS does the same at `complete_authorize_flow.rs:493-505` | **nothing** — just charge the verified nonce |

The Authorize builder branched purely on `authentication_data.is_some()`. Deleting the guard would
therefore have cross-wired a *Braintree-performed* authentication into the *external-MPI*
pass-through — UCS asserting an externally performed authentication for one Braintree performed
itself, which is exactly what §7.2 says must never happen.

**Sending nothing is correct, and that was proved live before it was coded.** A 3DS-verified nonce
charged with `chargeCreditCard` and **no** `threeDSecurePassThru` came back
`SUBMITTED_FOR_SETTLEMENT` with a fully populated `paymentMethodSnapshot.threeDSecure.authentication`
and `liabilityShifted: true`. Braintree attaches the authentication itself, because it lives on the
payment method.

The shipped discriminator is a four-variant `BraintreeAuthorizeThreeDsMode`
(`None` / `Hosted` / `ExternalPassThrough` / `Unauthenticated`) computed once from
`(is_three_ds, authentication_data, connector_feature_data)` and read at both the guard site and
the pass-through site, so there is one source of truth. `Hosted` is keyed on the
`braintree_three_ds` marker in `connector_feature_data`, behind a shared
`BRAINTREE_THREE_DS_FEATURE_KEY` const so producer and consumer cannot drift, and it **wins over**
`authentication_data`, because on the composite path both are present and the hosted authentication
is the one that actually happened. Rejected alternatives are recorded in the code so they are not
re-proposed: sniffing the token shape (`tokencc_` prefix vs bare UUID) is an undocumented string
heuristic on a credential; `is_three_ds()` alone is true for external MPI too; and a new
`PaymentsAuthorizeData` member would be a cross-crate change for something an already-forwarded
channel expresses exactly.

**The guard is narrowed, not deleted.** Deleting it outright would let a *direct*
`PaymentService/Authorize` call — one that never ran the trio — through with `is_three_ds()` and no
authentication, silently charging unauthenticated. Braintree-hosted 3DS still cannot happen inside
one Authorize call; that was true before this run and remains true. Only the `Unauthenticated`
variant now errors, and its message and `IntegrationErrorContext` were rewritten to point at the
trio instead of at a flow UCS does not have. The `!hosted` clause carries one real case: a
Braintree-hosted authentication that legitimately produced **no** `AuthenticationData` but did
produce the marker — without it, such a payment would be rejected after the trio had already run to
completion.

Both directions were exercised live over gRPC, not only in unit tests: the verified nonce with the
marker charged to `CHARGED` / `SUBMITTED_FOR_SETTLEMENT` with **zero** occurrences of
`passThrough` in the whole server log for the run; the same Authorize with the marker and
`authentication_data` both removed still fails closed.

### 8.6 Error handling

Unchanged in principle from §6.4 and §7.5, and for the same reason: Braintree answers HTTP 200 for
everything, so success and failure are separated by body shape. `BraintreePostAuthenticateResponse`
is an untagged enum with **`ErrorResponse` first**, and `GenericBraintreeResponse<T>` is again
deliberately **not** reused because it orders success first.

This leg adds one twist the prior two did not have: a `node(id:)` miss is a *partial* success —
`{"data":{"node":null},"errors":[…]}` — so neither a `data`-key-presence test nor a
`node != null` test would classify it correctly. Live, from the unknown-id case:
`failure_message "An object with this ID was not found."`, **`status: PENDING` with no terminal
status written** — the caller's own fallback, which is §P.7.1 / §P.8 behaviour and checklist item 5
demonstrated rather than asserted.

`CHALLENGE_REQUIRED` on the readback is **not** an error and is not converted into one. It maps to
`AuthenticationPending` (non-terminal) exactly as §7.4's table says, no terminal failure is
invented, no timeout is synthesised, and no `redirection_data` is emitted (the variant has no such
field). Because the readback is idempotent, the documented remedy is simply to call
`PostAuthenticate` again. Live-confirmed on the challenge card.

### 8.7 Appendix — reviewer-checklist audit (`grace/braintree_review_checklist.md`)

| Item | Applicable? | How it was satisfied |
|---|---|---|
| 1 — Currency is `common_enums::Currency` | No | The readback sends no currency and no amount; `node(id: ID!)` takes one argument. Nothing is stringified because nothing is sent. |
| 2 — amounts use an amount type | No | No amount on this leg — the amount belongs to leg 2's lookup and to Authorize. The connector's `amount_converter` is deliberately not invoked. |
| **3 — PII / credentials are `Secret<…>`** | **Yes** | The payment-method id is a spendable credential: `BraintreePostAuthenticateVariables.id` is `Secret<String>`, the readback's `node.id` is `Secret<String>`, and `cavv` is `Secret<String>` through the shared mapper. Proven by the live log line — the connector-response event printed `"id":"*** alloc::string::String ***"` and `"cavv":"*** alloc::string::String ***"`. |
| **4 — fixed-value strings become enums** | **Yes** | `usage` is modelled as a `BraintreePaymentMethodUsage` enum rather than a raw `String`. The status enums are leg 2's, reused verbatim. `eciFlag` stays `String` for the reason §7.6 gives (network-scoped values, and the existing pass-through code already treats it as one). |
| **5 — no hardcoded `Failure` in `build_error_response`** | **Yes** | The shared `ConnectorCommon::build_error_response` was not touched; the new error arm writes no status at all. Live-proven: the unknown-id case came back `PENDING`, not `Failure`. Leg 2's `TransactionStatus::default() == Failure` trap is inherited intact — the shared `to_trans_status` still returns `Option` and never `unwrap_or_default()`s. |
| **6 — unknown status → `Unspecified`** | **Yes** | The `#[serde(other)] Unknown → AttemptStatus::Unspecified` arm is leg 2's, reused rather than re-declared, and `post_authenticate_status_mapping_is_leg_twos_table_verbatim` asserts it on this leg's path. |
| **7 — terminal connector state → terminal UCS state** | **Yes** | Same table, same 15 reachable terminal statuses → `AuthenticationFailed`, and the test asserts `is_terminal_status()` on each. |
| **8 — do not map a state the pipeline cannot advance** | **Yes** | `CHALLENGE_REQUIRED → AuthenticationPending` is advanceable *because the readback is idempotent* — the caller can re-invoke this very leg (§8.6). Every `AuthenticationSuccessful` row is advanceable because the guard retirement (§8.5) now lets the verified nonce reach Authorize; before this run it could not, which is precisely what made the trio unusable. |
| 9 — partial capture | No | No capture on an authentication leg. |
| **10 — a 200 carrying a failure body becomes an `ErrorResponse`** | **Yes — and harder than on the prior two legs** | §8.6. A `node(id:)` miss is a *partial* success (`{"data":{"node":null},"errors":[…]}`), so `ErrorResponse` must be first in the untagged enum AND the success variant must not be satisfiable by a null node. `post_authenticate_errors_win_over_a_populated_data_object` and `post_authenticate_detects_every_empty_fragment_explicitly` (4 cases) pin both halves. Live-proven with a populated `code`/`message`/`reason`. |
| 11 — refund error paths set `attempt_status` | No | Not a refund flow. |
| **12 — Authorize and PSync return the same resource id** | **Partly — and §7.6 flagged it for this run** | §7.6 warned that leg 2's `resource_id` is a *payment-method* id, not a transaction id, and that the two must not be conflated. This leg keeps them apart: it echoes the payment-method id on `connector_response_reference_id` (surfacing as `merchant_order_id`), and the transaction id is minted later by Authorize and is what PSync syncs. Neither Authorize nor PSync was touched. |
| 13 — idempotency key from `get_merchant_request_id()` | No — **and this is the one leg where the concern genuinely evaporates** | `node(id:)` is a read. It consumes nothing, and two consecutive calls returned byte-identical data (live). No UUID is minted; a retry costs one HTTP request. Unlike leg 2, which is non-idempotent and destroys its input, this leg is safely re-runnable and pollable. |
| **14 — prefer the per-request field over the connector-config copy** | **N/A on this leg, for a reason worth recording** | `node(id: ID!)` takes one argument and no merchant scope — Basic auth *is* the scope. So the leg needs no `merchant_account_id`, `resolve_merchant_account_id` is deliberately not called, and `PaymentsPostAuthenticateData`'s missing `metadata` member does **not** force leg 2's cross-crate change. The id it does read comes from the per-request field `connector_order_reference_id` and from nowhere else, pinned by `post_authenticate_reads_connector_order_reference_id_and_not_payment_method_data`. The PR2187 reference implementation in the Authorize builder was not modified. |
| **15 / 16 — reuse existing helpers; generic logic in `utils.rs`** | **Yes — and this run paid down a clone rather than adding one** | `build_three_ds_authentication_data` was **extracted from leg 2 and is now shared by both legs**: both read the same `CreditCardDetails.threeDSecure.authentication` block, so they map it with one function instead of two copies that drift. Leg 2's behaviour was re-verified live after the refactor. Also reused: the `ThreeDSecureAuthenticationStatus` enum and its `From` impl, `ErrorResponse` / `ErrorDetails` / `AdditionalErrorDetails`, `build_error_response`, `utils::unexpected_response_fail`, and the existing `build_headers` / `connector_base_url_payments`. `GenericVariableInput<T>` was **not** reused — this document's variable is `{"id":…}`, not `{"input":…}`. Every new helper is Braintree-schema-specific, so nothing belonged in `utils.rs`. |
| 17 — no `billing_full_name` fallback for cardholder name | No | This leg sends no name and no address at all. |
| **18 — populated `IntegrationErrorContext`** | **Yes** | Every new error construction carries `additional_context` + `suggested_action`: the missing `connector_order_reference_id`, each empty-fragment case, and the rewritten Authorize guard (which also keeps its `doc_url`). On the response side `utils::unexpected_response_fail` populates `ResponseTransformationErrorContext`. No `::default()` in the new code. The missing-id message is quoted live in the gRPC evidence and names both the cause and the remedy. |
| **19 — comment non-obvious logic** | **Yes** | Comments on: why a *query* rather than a mutation (with the 112-mutation / 26-query counts in the code), why `redirect_response` is deliberately unread, why `createdAt` and `authenticationInsight` are not selected, why the `RedirectWithoutParams` arm is the ACS return, why Cybersource's dispatcher shape was chosen over Getnet's, why `Hosted` wins over `authentication_data`, the three rejected discriminator alternatives, why the guard is narrowed rather than deleted, why `ErrorResponse` is first in the untagged enum, and why `authentication_id` / `lookup_transaction_id` are dropped at the leg-2 → leg-3 hand-off. |
| **20 — novel local logic needs a `#[cfg(test)]` test** | **Yes — 9 new tests, 34 → 43, all green** | `post_authenticate_document_and_variables_key_agree` (both inline fragments, the `.authentication` nesting, and a byte-identical `authentication` selection shared with leg 2), `..._selects_neither_created_at_nor_authentication_insight` (literal negative assertions for the two live-captured footguns), `..._reads_connector_order_reference_id_and_not_payment_method_data`, `..._errors_win_over_a_populated_data_object`, `..._maps_a_settled_readback_onto_authentication_data`, `..._status_mapping_is_leg_twos_table_verbatim` (terminal / non-terminal / unknown → `Unspecified`), `..._detects_every_empty_fragment_explicitly` (4 cases), `next_authentication_step_routes_the_braintree_hosted_trio`, and `braintree_hosted_3ds_is_never_cross_wired_into_the_external_mpi_pass_through` (both directions plus the narrowed-guard cases). The sandbox and gRPC runs are evidence, not the test coverage. |
| **21 — never guess a production hostname** | **Yes** | No config file was touched. A GraphQL query is a POST to the same endpoint every other Braintree flow uses, resolved through the existing `connector_base_url_payments`. |
| **22 — no unrelated regenerated files** | **Yes** | `git status --porcelain` shows exactly three changed source files plus `data/integration-source-links.json`, which this run's Links Agent legitimately refreshed. `scripts/validation/pre-push.sh` was deliberately not run, so it could not regenerate `data/field_probe/braintree.json`, `docs-generated/**` or `examples/braintree/*` — nothing under those paths is dirty. `typos --config ./.typos.toml` clean, `cargo +nightly fmt --check` clean, `cargo clippy --all-targets` zero warnings, `check_connector_specs` 117/117. |

#### Credential hygiene

Re-verified over the whole **tracked** tree at the end of this run, per the operator's standing
requirement: the sandbox `public_key`, `private_key` and `metadata.merchant_id` from `creds.json`
have **zero** tracked hits. New test fixtures use the obviously-fake
`00000000-1111-2222-3333-444444444444`. One pre-existing echo is recorded rather than silently
churned: `merchant_account_id` is the literal string `juspay`, which leg 2's fixtures
(`transformers.rs:7034`, `:7083`) and the long-standing
`connector_specs/braintree/override.json` both contain. It is the organisation name, appears
~1640 times across the repo including `.github/CODEOWNERS`, and is not a secret — but it *is* a
`creds.json` value, so it is named here rather than left for a reviewer to find. The tech spec
does quote the sandbox merchant id; that file is gitignored (`grace/.gitignore:31
**/references/**`) and untracked, so it never reaches the tree.

### 8.8 Residual risks recorded by this audit

- **A real cardholder ACS challenge has still never been driven to completion in a browser.** The
  readback's live-view behaviour was proved by POSTing a synthetic PaRes to Braintree's `termUrl`,
  which produced `AUTHENTICATE_UNABLE_TO_AUTHENTICATE` — the *mechanism*, not a
  post-challenge CAVV. The frictionless path *was* observed carrying a real `cavv` through
  `node(id:)`, so the field is proven present on the readback surface; the specific
  challenge → CAVV transition is not. **First thing to check** when a browser challenge is
  exercised.
- **The cardholder may be stranded on Braintree's completion frame — this is the biggest
  end-to-end unknown.** Braintree 302s the browser to
  `assets.braintreegateway.com/3ds/…/html/authentication_complete_frame` with
  `frame-ancestors 'self'`. In the JS-SDK topology the SDK's iframe catches it; in UCS's
  server-side topology **nothing brings the cardholder back to the merchant automatically**. This
  leg is correct whenever it is invoked, and it is idempotent and pollable — but *what invokes it*
  is the caller's problem, and neither this run nor the spec can settle it. A caller rendering
  leg 2's `RedirectForm::Form` inside its own iframe and detecting navigation to the Braintree
  assets origin is the likely answer; it was not built or tested.
- **The dispatcher does not refuse to Authorize on a still-pending authentication.** The composite
  loop (`composite-service/src/payments.rs:874-884`) does not inspect PostAuthenticate's status —
  it sets `completed_step` and the next iteration returns `Authorize`. So a `CHALLENGE_REQUIRED`
  readback would be followed by a charge with no liability shift. The mitigation available
  entirely inside the connector is for the Authorize builder to refuse when the
  `braintree_three_ds` blob says the authentication is unsettled; whether to build it is a
  reviewer decision, deliberately not taken unilaterally because it may be better handled by the
  caller simply re-invoking this leg.
- **Braintree documents none of the server-side post-challenge path.** Everything in §8.1 is
  live-observed. It is not schema-gated, so it could in principle change without a
  `Braintree-Version` bump. Re-run the experiments if behaviour ever diverges.
- **`authentication_id` and `lookup_transaction_id` are lost at the leg-2 → leg-3 hand-off.**
  `node(id:)` returns neither, and the composite dispatcher replaces `connector_feature_data`
  *wholesale* rather than merging. Nothing reads them on the Authorize path today, so this is
  accepted — but it is a deliberate drop, not an oversight. If they are ever needed, the fix is to
  merge rather than replace, and it belongs in the connector, not the dispatcher.
- **The ACS form-field key casing (`PaReq` / `MD` / `TermUrl`) is now only half-inherited from
  §7.6.** This run POSTed `PaRes` + `MD` to Braintree's `termUrl` and got a real 302 with a real
  `auth_response`, so *Braintree's* side accepts that casing. What the ACS itself expects for
  `PaReq` is still browser-unobserved. Narrower than §7.6 recorded it, not gone.
- **The trio is still unreachable from hyperswitch.** HS's `braintree.rs` overrides none of the
  three `is_*_flow_required` methods (eleven other connectors do). Until that HS PR lands —
  scoped in `grace/braintree_hs_side_notes.md` — the legs are reachable only via UCS's own
  `CompositeAuthorize`. This is an HS-side gap, not a UCS one, and it is unchanged by this run
  except that §8.3's override now makes the UCS side complete.
- **`extensions.errorClass` is still not modelled.** Unchanged from §6.5 / §7.5, except that this
  run adds `NOT_FOUND` and `NOT_IMPLEMENTED` to the observed set. Still a shared-struct change
  across every Braintree flow, still out of a single-flow diff.
- **`AuthenticationInsight` is present on `ThreeDSecureDetails` and deliberately not selected** —
  it requires an `input` argument, and its three members are SCA-regulation advisory data with no
  `AuthenticationData` slot. A known, deliberate omission.
- **`strum::Display` vs serde spelling** on the pre-existing `BraintreePaymentStatus` — unchanged
  from §7.6, still deliberately untouched, still a papercut for whoever reads
  `connector_feature_data` on the Authorize path.

---

## 9. Network Transaction ID (NTID) for merchant-initiated transactions

**Scope**: the `RepeatPayment` flow, `Card` payment method — specifically whether a Braintree MIT
can carry the stored scheme network transaction id from the original CIT. Also touches the card
`Authorize` response path, because that is where the NTID is born. §6/§7/§8 (the 3DS trio) are
untouched by this run.

**Compared against** `/home/infamous/hyperswitch1/.../braintree.rs` and `.../braintree/transformers.rs`
(`main` @ `a2978004a4`), same as §1-§5.

### 9.1 The brief's premise was wrong twice over — and the answer to "which half exists" is *neither*

The run brief stated that `networkTransactionId` "already appears in braintree/transformers.rs
(~10 occurrences)" and asked which half of the NTID path was already wired — read (capturing the
NTID off the CIT) or write (sending it back on the MIT).

**Neither was wired. All ten occurrences were false positives.** Every one of them is a
`PaymentMethodData::CardDetailsForNetworkTransactionId(_)` or
`PaymentMethodData::DecryptedWalletTokenDetailsForNetworkTransactionId(_)` match arm inside a
"payment method not supported" catch-all. Those are UCS domain-type enum variants whose *names*
contain the substring; not one of them has anything to do with Braintree's GraphQL
`networkTransactionId`. The grep that produced the brief's count matched identifiers, not fields.

| Half | State before this run | Evidence |
|---|---|---|
| **READ** — capture the NTID off the CIT | **Absent.** No mutation in `mod constants` selected the field, so it was never even requested from Braintree. `network_txn_id` was hardcoded `None` at **11** `PaymentsResponseData::TransactionResponse` construction sites. | `transformers.rs:1505, 1542, 1602, 1896, 1940, 2001, 2771, 3222, 3348, 4230, 4536` |
| **WRITE** — send the NTID on the MIT | **Absent.** No `externalVault` anywhere in the file. `MandateTransactionBody` carried `amount`, `merchantAccountId`, `channel`, `orderId`, `paymentInitiator` and nothing else. `MandateReferenceId::NetworkMandateId` was unreachable: the builder accepted only `PaymentMethodData::MandatePayment` and read `connector_mandate_id()`, which returns `None` for that variant, so such a request died on `MissingRequiredField`. | `transformers.rs:385-392` (body), `:3946-3970` (builder), `connector_types.rs:3908-3916` (accessor) |

So this was not a half-built feature to finish. It was absent in both directions.

### 9.2 Three field names in circulation were wrong — two of them fatally

The brief, and this run's own Links Agent, both supplied field names that **do not exist**. Live
introspection against the sandbox at `Braintree-Version: 2019-01-01` (785 types) settled each one
before a line of code was written. Had any been implemented as supplied, the result would have
been a 100% failure rate, not a subtle bug.

| Claimed | Verdict | What it actually is |
|---|---|---|
| `previousNetworkTransactionId` | **Does not exist.** Variable coercion rejects it: *"contains a field name 'previousNetworkTransactionId' that is not defined for input object type 'TransactionExternalVaultOptionsInput'"* | **`verifyingNetworkTransactionId`** |
| `TransactionInput.externalVault` | **Not on `TransactionInput`.** Its 23 fields do not include it; a referrer sweep over all 785 types found exactly one attachment point. | **`input.options.externalVault`**, on `CreditCardTransactionOptionsInput` — a **sibling** of `input.transaction`, not a member of it |
| `transactionSource` | **Does not exist at any version**, and there is no `enum TransactionSource` in the schema. It is the legacy REST/server-SDK name. | **`transaction.paymentInitiator`** (enum `PaymentInitiator`, 8 members) — which the repo already sent correctly |

The third is worth stating positively: **the repo was already right about `paymentInitiator`**, and
this run changed nothing about it. §NT.2.1 of the tech spec carries the verbatim 23-field
`TransactionInput` and the reproduction script.

The read path's traversal was equally unobvious. `Transaction` has 43 fields and
`networkTransactionId` is **not** one of them — selecting it there is a document-level
`FieldUndefined` validation error, which rejects the whole document *before execution* and would
therefore have broken **every** Authorize, not just MIT ones. The sole carrier in the entire schema
is `CreditCardTransactionDetails.networkTransactionId` (non-deprecated at the pin), reached through
the union `Transaction.paymentMethodSnapshot`, which makes the inline fragment mandatory:

```graphql
paymentMethodSnapshot { ... on CreditCardTransactionDetails { networkTransactionId } }
```

The union also contains `CreditCardDetails`, which carries no such field — a plausible-looking
`... on CreditCardDetails` would validate cleanly and silently return nothing forever.

### 9.3 RULE NT-1 — the regime split, and why the two halves are not symmetric

The decisive finding is a single sentence in the live schema's own description of
`TransactionExternalVaultOptionsInput`:

> "Input for transactions created with credit cards vaulted in an external vault, not the Braintree
> Vault. **Do not use for transactions created from Braintree multi-use payment methods**, or from
> single-use payment methods which will not be stored in an external vault."

Braintree's RepeatPayment in this repo charges a Braintree multi-use payment method — the vaulted
`paymentMethod.id` minted by `vaultPaymentMethodAfterTransacting` on the CIT and replayed as
`connector_mandate_id`. **That is precisely the case the schema names.** A naive "read it on
Authorize, write it back on RepeatPayment" implementation would therefore have been wrong on the
only path the connector actually exercises.

> **RULE NT-1.** `options.externalVault` is emitted **if and only if** the credential is vaulted
> **outside** Braintree.
>
> - **Regime A — Braintree-vaulted.** `externalVault` MUST be omitted; Braintree replays the
>   stored-credential chain itself and the entire MIT signal is `paymentInitiator: UNSCHEDULED`,
>   already sent. **The write half is a no-op here.**
> - **Regime B — externally vaulted (UCS/HS is the vault of record).** `externalVault =
>   { status: VAULTED, verifyingNetworkTransactionId: <CIT NTID> }` alongside
>   `paymentInitiator: UNSCHEDULED`.

**And no test can catch a violation.** The sandbox accepts `externalVault` on a Braintree-vaulted
token, accepts it with no NTID, and accepts `verifyingNetworkTransactionId: "NOT-A-REAL-NTID"` —
all three return `SUBMITTED_FOR_SETTLEMENT`. There is no API-observable difference between a
correctly chained NTID and one silently ignored; the difference shows up only in scheme-level
interchange qualification and issuer decline rates. The only constraints the gateway *does* enforce
are `WILL_VAULT` + NTID (hard error) and `externalVault` without `status` (hard error).

Correctness here had to be achieved **by construction**, which is why the regime is a type:
`BraintreeMitVaultRegime::BraintreeVaulted` returns `None` from `external_vault()` for every
possible input (`transformers.rs:948-958`), and `MandatePaymentRequest::try_from` takes that enum
rather than a bare token string, so Regime A cannot emit `externalVault` even by accident.

### 9.4 What shipped

One file, `braintree/transformers.rs`, +645/−21, 43 → **55** tests.

- **Read.** The snapshot fragment added to exactly the four card mutations
  (`CHARGE_CREDIT_CARD_MUTATION`, `AUTHORIZE_CREDIT_CARD_MUTATION`,
  `CHARGE_AND_VAULT_TRANSACTION_MUTATION`, `AUTHORIZE_AND_VAULT_CREDIT_CARD_MUTATION`) — verified
  present in all four and absent from all eight wallet mutations, whose snapshot resolves to a
  non-card union member and would always yield `None`. `network_txn_id` now populated at the card
  Authorize (both mutations) and RepeatPayment sites; **not gated on success status**, because the
  NTID is assigned at authorization time and is live-observed present even on `PROCESSOR_DECLINED`.
  `network_txn_link_id` deliberately left `None` — that is the Mastercard TLID slot and Braintree
  exposes no equivalent.
- **CIT→MIT hand-off.** `MandateReferenceId` is an enum, so a Braintree MIT — which needs the vault
  token *and* the NTID simultaneously — cannot get both from it. The NTID rides in
  `ConnectorMandateReferenceId.mandate_metadata` as `BraintreeMandateMetadata`.
- **Write.** `external_vault` on `CreditCardTransactionOptions` **and on its `is_empty()`** — the
  latter is the silent-drop trap: without it the whole `options` object is discarded and the MIT
  quietly loses its NTID with no error. Modelled as `#[serde(tag = "status")]` with `WillVault` a
  unit variant, so the gateway-rejected pairing is unrepresentable and `status` is always emitted.

### 9.5 A stale inherited caveat, corrected and disproved

The tech spec's plumbing section inherited a caveat from Paysafe
(`paysafe/transformers.rs:98-106`): *"the gRPC recurring path cannot carry `mandate_metadata`"* —
which is why Paysafe JSON-encodes both values into `connector_mandate_id` instead. Taken at face
value it would have made the entire hand-off dead on the primary path.

**It is stale.** `payment.proto:1412-1417` gives `ConnectorMandateReferenceId` an
`optional SecretString mandate_metadata = 4`; the producer (`domain_types/src/types.rs:6867-6884`)
writes it and the consumer (`:6886-6910`) reads it back on the `ConnectorMandateId` branch; and
`RepeatPaymentData::foreign_try_from` routes through exactly that conversion. Proved live rather
than argued: a CIT Authorize returned

```json
"mandateReference": { "connectorMandateId": {
    "connectorMandateId": "cGF5bWVudG1ldGhvZF9jY18wdzI4d3RkNg",
    "mandateMetadata": { "value": "{\"network_transaction_id\":\"020260915131905\"}" } } }
```

and feeding that pair straight back into `RecurringPaymentService/Charge` succeeded. So
`mandate_metadata` is used directly and Paysafe's workaround was **not** copied. The defensive
degradation is kept regardless — absent, null, foreign-shaped or unparsable metadata all yield "no
NTID" and never error, because in Regime A that degradation is a no-op and an error there would
break the existing working path.

### 9.6 Live evidence

Sandbox, `Braintree-Version: 2019-01-01`, four real transactions:

| # | What | Result |
|---|---|---|
| 1 | Card Authorize | `networkTransactionId: 020260915131828` — **a field this connector had never once requested before** |
| 2 | Vault CIT | NTID `020260915131905` returned *and* carried out over gRPC in `mandateMetadata` |
| 3 | **Regime A** MIT (vault token + metadata NTID present) | `CHARGED`. Outbound body, from the server log: `{"paymentMethodId":"…","transaction":{"amount":"12.00","merchantAccountId":"…","channel":"HyperSwitchBT_Ecom","orderId":"grace_ntid_mit_001","paymentInitiator":"UNSCHEDULED"}}` — **no `options` key at all.** The NTID was available and deliberately not sent. Byte-for-byte the pre-change body. |
| 4 | **Regime B** MIT (`NetworkMandateId` + single-use token) | `CHARGED`. `"options":{"externalVault":{"status":"VAULTED","verifyingNetworkTransactionId":"***"}}` as a **sibling** of `transaction`. This request returned `NotSupported` before the change. |

Row 3 is the one that matters: it is the empirical proof of RULE NT-1 under the exact condition
that would tempt a wrong implementation — the NTID present and ignored.

### 9.7 Parity: hyperswitch does **not** carry an NTID on Braintree MIT, in any form

The brief framed this as a gap where UCS trailed hyperswitch. It is the opposite, and the earlier
run's §2 finding repeats here. Checked empirically at `a2978004a4`:

| Probe | hyperswitch result |
|---|---|
| `grep -rl 'externalVault\|previousNetworkTransactionId'` across **all** of `hyperswitch_connectors/src/connectors/` | **zero files** — not Braintree, not any of the ~120 other connectors |
| `paymentMethodSnapshot` in `braintree/transformers.rs` | **0 occurrences** — the NTID is never requested |
| `network_txn_id` in `braintree/transformers.rs` | `None` at **all 11** sites (`:791, 818, 852, 1015, 1042, 1086, 1161, 1235, 2134, 2342, 2473`) |
| `mandate_metadata` in `braintree/transformers.rs` | `None` at **all 5** `MandateReference` sites (`:786, 1010, 1081, 1156, 1230`) |
| `PaymentInitiatorType` | `{ Unscheduled, RecurringFirst }` — the same two variants UCS had |
| `MandateTransactionBody` | structurally identical to UCS's pre-change body |
| `RepeatPayment` flow | **does not exist**; HS routes MIT through `Authorize` + `MandatePayment` |

**Stated plainly: this is net-new capability in UCS, not a parity catch-up.** There was no
hyperswitch implementation to diff against, so nothing in §9 is a port. The one thing the two sides
did share — `paymentInitiator` on the MIT — was already correct on both.

### 9.8 Appendix — reviewer-checklist audit (`grace/braintree_review_checklist.md`)

| Item | Applicable? | How it was satisfied |
|---|---|---|
| 1 — Currency is `common_enums::Currency` | **Yes** | No currency field was added. The existing `validate_currency(request.currency, Some(metadata.merchant_config_currency))` in the RepeatPayment builder is untouched and still takes `enums::Currency` on both sides. Nothing was stringified. |
| 2 — amounts use an amount type | **Yes** | No amount field was added. `MandateTransactionBody.amount` remains `StringMajorUnit` via the connector's `amount_converter`, unchanged — pinned byte-for-byte by `regime_a_request_body_is_unchanged_by_ntid_support` (`"amount":"12.00"`). The NTID is not an amount and is never numeric. |
| **3 — PII / credentials are `Secret<…>`** | **Yes** | `verifying_network_transaction_id` is `Option<Secret<String>>` and `single_use_token` is `Secret<String>`. Live-proven: the Regime B outbound log line printed `"verifyingNetworkTransactionId":"*** alloc::string::String ***"`. `BraintreeMandateMetadata.network_transaction_id` is deliberately plain `String` — it is serialized into `mandate_metadata`, which is itself a `SecretSerdeValue`, so the wrapping happens one level up; double-wrapping would have put a masked literal inside the JSON. §NT.10 records that the response-side slot (`network_txn_id: Option<String>`) is unwrapped by the shared type's own definition, so full secrecy is not achievable regardless. |
| **4 — fixed-value strings become enums** | **Yes** | `ExternalVaultStatus` is modelled as the two-member enum the schema declares (`VAULTED`, `WILL_VAULT`) via `#[serde(tag = "status")]`, never a free-form `String` — asserted by `external_vault_status_is_always_present_and_never_a_free_string`. Because `status` is `ExternalVaultStatus!` (NON-NULL) it is never `Option` and never skipped. |
| **5 — no hardcoded `Failure` in `build_error_response`** | **Yes** | The shared `ConnectorCommon::build_error_response` was not touched, and no new error arm writes a status. The NTID read path adds no error arm at all — an absent snapshot yields `None`, never an error. |
| **6 — unknown status → `Unspecified`** | **Yes** | Status mapping was not modified; `BraintreePaymentStatus`'s `#[serde(other)] Unknown → AttemptStatus::Unspecified` arm is intact and still covered by `an_unrecognised_braintree_status_is_unspecified_not_an_invented_terminal_state`. The snapshot is a sibling of `status` in the selection set and cannot perturb it. |
| 7 — terminal connector state → terminal UCS state | **Yes (unchanged)** | No status transition was added or altered. |
| **8 — do not map a state the pipeline cannot advance** | **Yes** | Regime B was previously unreachable (`NotSupported`); it now completes to a real terminal status, which *removes* a dead end rather than creating one. Regime A's states are unchanged. |
| 9 — partial capture reports `PartialCharged` | No | No capture logic touched. **But see the deviation in §9.9** — the Capture response body carries no snapshot, so `network_txn_id` stays `None` there. |
| **10 — a 200 carrying a failure body becomes an `ErrorResponse`** | **Yes** | Untouched and deliberately not weakened: the NTID is read from the same `TransactionAuthChargeResponseBody` that already routes through `is_payment_failure` → `create_failure_error_response`. Because the accessor is **not** status-gated, a declined transaction still produces an `ErrorResponse` *and* the NTID is still captured — `network_transaction_id_is_not_gated_on_a_success_status` pins exactly this, and it matters because Braintree assigns the NTID at authorization time, decline included. |
| 11 — refund error paths set `attempt_status` | No | No refund path touched; the refund `network_txn_id` sites remain `None`. |
| **12 — Authorize and PSync return the same resource id** | **Yes** | `resource_id` was not touched on any flow. The NTID goes to `network_txn_id`, a distinct slot, and the vault token continues to go to `mandate_reference.connector_mandate_id`. Live rows 1-4 all show `connectorTransactionId` unchanged in shape, and PSync still anchors on the Braintree transaction id. |
| **13 — idempotency key from `get_merchant_request_id()`** | **Yes** | `PaymentInput.api_request_key` still comes from `resource_common_data.get_merchant_request_id()` in the MIT builder; no UUID is minted anywhere in the new code. Critically, Braintree's own contract is that a repeated `apiRequestKey` **must carry identical input** — so silently adding `options.externalVault` to Regime A would have broken idempotent retries as well as scheme semantics. RULE NT-1 protects this too. |
| **14 — prefer the per-request field over the connector-config copy** | **Yes — and explicitly preserved** | PR2187 cites this very file as the reference implementation, so it must not regress. The RepeatPayment builder still reads `request.merchant_account_id` / `request.merchant_configured_currency` first and falls back to `BraintreeAuthType` only when absent; that block is byte-identical. The new NTID lookup follows the same discipline: the per-request `MandateReferenceId::NetworkMandateId` is consulted **first**, with `mandate_metadata` only as the secondary carrier. |
| **15 / 16 — reuse existing helpers; generic logic in `utils.rs`** | **Yes** | Reused: `is_auto_capture()` for the mutation choice, `validate_currency`, `get_merchant_request_id()`, `get_mandate_metadata()`, the existing `CreditCardTransactionOptions` + its `is_empty()` discipline, `GenericBraintreeRequest` / `VariablePaymentInput` / `PaymentInput`, and `TransactionAuthChargeResponseBody` (extended, not cloned — the same struct now serves card Authorize and RepeatPayment, so the two cannot drift). Nothing was hand-rolled that already existed. Nothing belonged in `utils.rs`: every new type names a Braintree GraphQL input or output (`TransactionExternalVaultOptions`, `ExternalVaultStatus`, `CreditCardTransactionSnapshot`, `BraintreeMandateMetadata`) or encodes a Braintree-specific rule (`BraintreeMitVaultRegime`). |
| 17 — no `billing_full_name` fallback for cardholder name | No | No name or address field is sent on either regime's MIT. |
| **18 — populated `IntegrationErrorContext`** | **Yes** | No `::default()` in new code. The point is largely moot by design, though, and that is deliberate: the NTID paths are built to **degrade, not error** (§9.5), so there are few new error constructions to populate. The one genuinely new failure mode — Regime B reached without a token — reuses the existing `raw_card_not_tokenized_error()`, which already carries `additional_context`, `suggested_action` and `doc_url`. |
| **19 — comment non-obvious logic** | **Yes** | Comments on: RULE NT-1 and both regimes at the enum (`:911-931`), `external_vault()` annotated as "RULE NT-1, the whole of it" (`:947`), why `verifyingNetworkTransactionId` and not `previousNetworkTransactionId` (`:4256-4257`), why `status` is never `Option`, why `WillVault` is a unit variant, why the NTID in `mandate_metadata` is plain `String`, the two-carrier lookup order, and why the read is not status-gated. |
| **20 — novel local logic needs a `#[cfg(test)]` test** | **Yes — 12 new tests, 43 → 55, all green** | This item is doubly binding here because §9.3 proves **no live test can catch a regime error**. The by-construction invariants are therefore pinned in-repo: `regime_a_can_never_emit_external_vault`, `regime_a_request_body_is_unchanged_by_ntid_support` (byte-for-byte JSON), `regime_b_emits_external_vault_under_options_not_transaction`, `regime_b_without_an_ntid_degrades_instead_of_failing`, `external_vault_status_is_always_present_and_never_a_free_string`, `options_is_empty_accounts_for_external_vault` (the silent-drop trap), `network_transaction_id_is_read_through_the_snapshot_union`, `an_unmatched_snapshot_union_member_yields_none_rather_than_failing`, `network_transaction_id_is_not_gated_on_a_success_status`, `card_mutations_select_the_snapshot_and_never_the_undefined_transaction_field` (asserts one occurrence per card mutation — i.e. only inside the fragment, never bare on `Transaction` — and zero on all eight wallet mutations), `mandate_metadata_round_trips_the_network_transaction_id`, `a_missing_or_unusable_mandate_metadata_degrades_to_no_ntid`. The sandbox run is evidence, not coverage. |
| **21 — never guess a production hostname** | **Yes** | No config file touched. Both regimes POST to the same `/graphql` endpoint every other Braintree flow already uses, through the existing `connector_base_url_payments`. |
| **22 — no unrelated regenerated files** | **Yes** | `git status --porcelain` shows exactly **one** changed tracked file, `braintree/transformers.rs`, plus this report. `data/integration-source-links.json` was regenerated mid-run and reverted — see §9.9. `cargo +nightly fmt` clean, `cargo clippy --package connector-integration --all-targets` warning-free, `typos --config ./.typos.toml` clean. |

#### Credential hygiene

Re-verified over the whole **tracked** tree after the change, per the operator's standing
requirement. `braintree.public_key`, `braintree.private_key` and `braintree.metadata.merchant_id`:
**zero tracked hits, and zero occurrences in the diff.** Every new test literal is an obviously-fake
placeholder (`111111111111111`, `cGF5bWVudG1ldGhvZF9mYWtlXzAwMDA`,
`tokencc_fake_0000_0000_0000_000`, `fake_merchant_account`). The one pre-existing echo §8.7 recorded
is unchanged and was not re-introduced by this run: `merchant_account_id` is the literal string
`juspay`, the organisation name, appearing in 146 tracked files including `.github/CODEOWNERS`.
This run's prior failure mode — codegen hardcoding sandbox values into fixtures — did not recur.

### 9.9 Residual risks and deviations recorded by this audit

- **A wrong NTID is undetectable through the API, permanently.** This is the single most important
  thing a reviewer should carry away. §9.3's three probes show the gateway accepting a malformed
  NTID, a missing NTID and a forbidden regime with identical `SUBMITTED_FOR_SETTLEMENT` responses.
  No green suite — here or in CI or in production smoke tests — can distinguish correct chaining
  from silent discard. Only interchange qualification and issuer decline rates will. **Any future
  change to the regime split needs scheme-level sign-off, not a passing test run.**
- **Regime B is reachable but has never been driven by a real caller.** It is exercised only by this
  run's hand-built grpcurl (§9.6 row 4). It requires the caller to have run
  `PaymentMethodService/Tokenize` first and to present `MandateReferenceId::NetworkMandateId` —
  a combination no orchestrator currently emits. Whether UCS should *be* the vault of record for
  Braintree is a product decision this run deliberately did not take; the tech spec's §NT.10 records
  it as the central open question, and the recommendation there was to ship the read path and gate
  the write path behind that decision. **The write path is shipped but inert until a caller opts in**,
  which is the reversible position.
- **Deviation — the Capture site still reports `network_txn_id: None`.** The codegen brief asked for
  the card Authorize / Capture / RepeatPayment sites. Capture deserializes
  `CaptureResponseTransactionBody` (`{id, status}`) off `CAPTURE_TRANSACTION_MUTATION`, which is not
  one of the four card transaction mutations and exposes no `paymentMethodSnapshot`. Populating it
  was impossible without widening the mutation list; that was flagged rather than done silently.
  Low impact — the NTID is assigned at authorization, so Authorize already captures it — but it is a
  deliberate omission, not an oversight.
- **Deviation — Regime B accepts `mandate_metadata` as a secondary NTID carrier.** Strictly, Regime B
  is keyed on `NetworkMandateId`, which carries its own NTID. But `mandate_metadata` exists only on
  the `ConnectorMandateId` branch, so without this fallback the §9.4 read-back would have had no
  consumer at all on any path. The lookup order is `NetworkMandateId` first (the per-request,
  PSP-agnostic slot — checklist #14), `mandate_metadata` second. Regime A is unaffected either way
  and structurally still cannot emit `externalVault`.
- **`data/integration-source-links.json` was reverted, losing this run's Links Agent refresh.** A
  build regenerated the file mid-run and the codegen agent reverted it to keep the diff to one file
  (checklist #22). The net effect is that Phase 1's refresh from 14 to 15 verified links is **not**
  in this commit — a divergence from §8.7, where the equivalent refresh was legitimately included.
  Nothing depends on it: every URL it would have added is already quoted in the tech spec's §NT.
  Recorded so a reviewer does not read its absence as the Links Agent having failed.
- **The four empty arrays in `x-connector-config` are load-bearing.** `BraintreeConfig`'s `repeated`
  fields (`apple_pay_supported_networks`, `apple_pay_merchant_capabilities`,
  `gpay_allowed_auth_methods`, `gpay_allowed_card_networks`) have no `#[serde(default)]`, so omitting
  them fails header parsing with *"Failed to parse X-Connector-Config JSON into
  ConnectorSpecificConfig"* — an error that names neither the field nor the reason. Unrelated to
  NTID, but it cost time in this run and will cost it again; worth a `#[serde(default)]` in a
  separate PR.
- **`network_txn_link_id` remains `None` everywhere, deliberately.** That is the Mastercard TLID /
  `transactionLinkId` slot. Braintree exposes no equivalent at the pin, and populating it with the
  NTID would conflate two distinct scheme identifiers.
- **Four fields on the newly-selected snapshot are free but unconsumed.**
  `acquirerReferenceNumber` (the usual chargeback-reconciliation key),
  `processedWithCardOnFileNetworkToken`, `accountType` and `accountBalance` now sit one selection
  away at no extra cost. No UCS slot was identified for any of them; noted so the next reader does
  not re-discover the fragment from scratch.
- **The `Braintree-Version: 2019-01-01` pin is untouched**, and §9 adds a second independent reason
  to keep it: everything specified here is already present at that version, so there is no NTID
  argument for raising it. §7.1's `CountryCode` alpha-3 → alpha-2 hazard at 2021-02-01 stands
  unchanged.
