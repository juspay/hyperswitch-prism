# Adyen Paze + SamsungPay — Intentional Deviations from Hyperswitch

Scope: Adyen `Authorize` and `SetupRecurring` (internally `SetupMandate`) for the Paze
and SamsungPay wallets, on branch `feat/adyen_grace`.

Reference implementation: `~/hyperswitch5`,
`crates/hyperswitch_connectors/src/connectors/adyen{.rs,/transformers.rs}`.

Every item below is a **deliberate** difference from the reference. Defects found during
parity review were fixed in `07caf924b` and are not listed here.

---

## D1 — `mpiData.tokenAuthenticationVerificationValue` carries the TAVV cryptogram, not the PAR

**Reference** (`adyen/transformers.rs:4083-4085`) sends
`paze_data.token.payment_account_reference`.
**UCS** sends the `CRYPTOGRAM_3DS` entry from `paze_decrypted_data.dynamic_data`.

**Rationale.** The Adyen field is defined as the token authentication verification value —
the network token cryptogram. The Payment Account Reference is a stable card identifier,
not an authentication value; sending it asserts nothing about the transaction. The
reference's use of the PAR here appears to be a latent bug rather than an intended mapping.
UCS's own Cybersource connector and the reference's Cybersource connector both source the
cryptogram from the decrypted payload for the equivalent field.

**Status: DELIBERATE — needs a product decision before shadow validation runs.**
This field will diff on **every** Paze transaction. Verified live: Adyen returns `CHARGED`
with the cryptogram. If parity with the router is required over correctness, revert
`build_paze_mpi_data` to `paze_decrypted_data.token.payment_account_reference`.

---

## D2 — `paymentMethod.expiryYear` is expanded to four digits

**Reference** forwards `token_expiration_year` verbatim (2-digit when Paze supplies 2).
**UCS** passes it through `domain_utils::expand_expiry_year_to_four_digits`.

**Rationale.** Adyen's networkToken schema documents a four-digit `expiryYear`. Expanding
is the safer normalization and matches how UCS handles expiry across all other connectors.
Low risk: for a 4-digit input the helper is an identity.

---

## D3 — `holderName` has an extra `consumer.full_name` fallback

**Reference** resolves holder name from the Paze billing address only.
**UCS** falls back: Paze `billing_address.name` → request billing full name →
`consumer.full_name`.

**Rationale.** Adyen rejects network-token authorizations with an absent holder name for
some card brands. The Paze `consumer` block is always populated, so the fallback converts
an avoidable decline into a successful authorization. Serialization now uses
`skip_serializing_if`, so the field is omitted (never `null`) when all three are absent.

---

## D4 — Paze is submitted as `paymentMethod.type: "networkToken"`

Not a deviation — recorded here because it was initially reported as one.

The reference's `AdyenPaze(Box<AdyenPazeData>)` variant carries
`#[serde(rename = "networkToken")]` (`adyen/transformers.rs:809-810`). It is itself a
networkToken pass-through. UCS reuses the existing `AdyenPaymentMethod::NetworkToken`
variant and is **wire-equivalent**. Adyen exposes no Paze-native `paymentMethod.type`
(verified: docs 404, zero `paze` hits across Checkout OpenAPI v68–v72).

---

## D5 — SamsungPay: no deviation

UCS `AdyenSamsungPay` and reference `SamsungPayPmData` serialize identically:
type tag `"samsungpay"`, field `"samsungPayToken"`, both forwarding
`payment_credential.token_data.data` (proto `3_d_s`), both omitting `mpiData`.

---

# Known divergences NOT fixed (pre-existing, out of scope)

These are inherited by the new wallet paths but were introduced by neither the four GRACE
commits nor `07caf924b`. They affect cards identically and are recorded for follow-up.

| # | Divergence | Location | Impact |
|---|---|---|---|
| P1 | SetupRecurring sends `request.amount.unwrap_or(0)`; the reference hard-codes `0` | `adyen/transformers.rs` `get_amount_data_for_setup_mandate` | A caller passing a non-zero amount performs a **real charge** instead of a zero-value verification |
| P2 | SetupMandate error responses are not parsed into Adyen's error envelope | shared SetupMandate plumbing | Authorize surfaces `errorCode 11_006`; SetupRecurring surfaces `500 / internal_server_error` with the body unparsed. Confirmed live, both wallets |
| P3 | `SetupRecurringRequest → SetupMandateRequestData` reads `customer.connector_customer_id`; the sibling impl at `types.rs:11162` reads `customer.id` | `types-traits/domain_types/src/types.rs:4318` | SetupRecurring fails `MISSING_REQUIRED_FIELD: customer_id` unless `connector_customer_id` is set. Reproduces with plain cards |
| P4 | `applicationInfo` dropped on the UCS SetupMandate path | `adyen/transformers.rs` SetupMandate builder | Partner attribution lost vs. the reference |
| P5 | `PaymentFlowData.payment_method` hard-coded to `PaymentMethod::Card` on the live Authorize and SetupRecurring paths | `types.rs:4966`, `types.rs:5062` | Every wallet is reported to downstream logic as a card. This is why the missing `PazeSdk` arm was latent rather than fatal |
| P6 | ~90-line duplicate between the card and wallet SetupMandate request builders | `adyen/transformers.rs` | Maintenance risk; flagged by review-theme T5 |

---

# Verification status

| Scenario | Result |
|---|---|
| Authorize + Paze (ECI present) | ✅ `CHARGED` — live Adyen sandbox |
| Authorize + Paze (ECI absent) | ✅ `CHARGED` — confirms the removed `"05"` default was never required |
| SetupRecurring + Paze | ✅ `CHARGED`, zero-value auth, recurring id returned |
| Authorize + SamsungPay | ⚠️ Reaches Adyen, refused at token decryption (`11_006`) |
| SetupRecurring + SamsungPay | ⚠️ Reaches Adyen, refused at token decryption (`11_006`) |

SamsungPay cannot be closed without a genuine Samsung device SDK payload; Adyen publishes
no static test token. Both refusals returned real `pspReference` values and none of the
request-shape errors (`14_015`, `14_007`, `14_394`), confirming the payload shape, the
type tag, and merchant-account enablement are correct.

**Not verified:** router-data shadow validation never ran end-to-end (the hyperswitch
router was never built and the root disk is full), and `data/field_probe/adyen.json` +
`docs-generated/` could not be regenerated for the same reason. See
`grace/adyen_shadow_validation_report.md`.
