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
