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
