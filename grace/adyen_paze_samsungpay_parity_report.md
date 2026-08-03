# Adyen Paze + Samsung Pay — UCS vs. hyperswitch5 Parity Report

**Branch under review:** `feat/adyen_grace` (`45351c251..HEAD`)
**UCS repo:** `/home/infamous/hyperswitch-prism5`
**Reference:** `/home/infamous/hyperswitch5` @ `63cfcc3e5e`
 · `crates/hyperswitch_connectors/src/connectors/adyen.rs`
 · `crates/hyperswitch_connectors/src/connectors/adyen/transformers.rs`

**Commits reviewed**

| SHA | Subject | Scope |
|---|---|---|
| `8a6d97cc5` | feat(connector): implement Authorize (Paze) for adyen | Authorize / Paze |
| `9aac731f7` | wip(connector): **[FAILED]** implement Authorize for adyen | Authorize / Samsung Pay |
| `8b8d53fbc` | chore: auto-fix formatting and generated code | field probe + docs regen |
| `9e3c4de32` | feat(connector): implement SetupRecurring for adyen | SetupMandate / Paze |
| `1b1c832d9` | wip(connector): **[FAILED]** implement SetupRecurring for adyen | SetupMandate / Samsung Pay |

Note that the generated artefacts (`data/field_probe/adyen.json`, `docs-generated/**`) were regenerated at
`8b8d53fbc`, which is **before** both SetupRecurring commits. They therefore describe the Authorize
flow only.

---

## Answers to the two high-priority questions

### A. What `paymentMethod.type` does the reference serialize for `AdyenPazeData`?

**`"networkToken"`.** Verbatim from the reference:

```rust
// hyperswitch5 crates/hyperswitch_connectors/src/connectors/adyen/transformers.rs:809-810
#[serde(rename = "networkToken")]
AdyenPaze(Box<AdyenPazeData>),
```

`AdyenPazeData` (reference `transformers.rs:1439-1449`) is `#[serde_with::skip_serializing_none]`
+ `#[serde(rename_all = "camelCase")]` with fields
`number`, `expiry_month`, `expiry_year`, `cvc`, `holder_name`, `brand`, `network_payment_reference`.
Construction (reference `transformers.rs:2701-2721`) sets `cvc: None` and
`network_payment_reference: None`, both of which are then omitted by `skip_serializing_none`.

The reference wire payload is therefore:

```json
{"type":"networkToken","number":"…","expiryMonth":"…","expiryYear":"…","holderName":"…","brand":"…"}
```

The UCS implementation routes Paze into the pre-existing `AdyenPaymentMethod::NetworkToken`
variant, which carries `#[serde(rename = "networkToken")]`
(`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:219-220`) and
serialises `AdyenNetworkTokenData` (`transformers.rs:182-193`) with exactly the same camelCase field
names (it simply has no `cvc` field at all, which is wire-identical to the reference's skipped
`cvc: None`).

> **VERDICT (A): the UCS approach is wire-equivalent to the reference for the `paymentMethod`
> object.** The `AdyenPaze` variant in the reference is *itself* nothing more than a
> `type: networkToken` pass-through — the "Adyen has no Paze-native type" justification in the UCS
> comment is factually correct and matches what the reference actually does. This is **not** a
> divergence.
>
> The real divergences on the Paze path are **outside** the `paymentMethod` object — in `mpiData`
> (see DEV-02, DEV-03) — and, more seriously, the whole Paze path is currently unreachable
> (DEV-01).

Field-by-field comparison of the Paze `paymentMethod` object:

| Field | Reference | UCS | Verdict |
|---|---|---|---|
| `type` | `"networkToken"` | `"networkToken"` | identical |
| `number` | `token.payment_token` | `token.payment_token` | identical |
| `expiryMonth` | `token.token_expiration_month` | `token.token_expiration_month` | identical |
| `expiryYear` | raw `token.token_expiration_year` | `expand_expiry_year_to_four_digits(...)` | DEV-04 (benign) |
| `cvc` | `None` → omitted | field does not exist | identical on wire |
| `holderName` | `billing_address.name` → `billing_full_name` | + third fallback `consumer.full_name` | DEV-05 |
| `brand` | `get_adyen_card_network(payment_card_network)` | `get_adyen_card_network(payment_card_network)` | identical |
| `networkPaymentReference` | `None` → omitted | `None` → omitted | identical |

### B. Samsung Pay — variant string, field name, source field

| | Reference | UCS |
|---|---|---|
| Enum variant | `SamsungPay(Box<SamsungPayPmData>)` (`transformers.rs:884`), no explicit rename → inherits enum-level `#[serde(rename_all = "lowercase")]` → **`"samsungpay"`** | `SamsungPay(Box<AdyenSamsungPay>)` with explicit `#[serde(rename = "samsungpay")]` (`transformers.rs:228-229`) → **`"samsungpay"`** |
| Struct | `SamsungPayPmData` (`transformers.rs:980-983`) | `AdyenSamsungPay` (`transformers.rs:1095-1099`) |
| Field serde name | `#[serde(rename = "samsungPayToken")]` | `#[serde(rename = "samsungPayToken")]` |
| Source field | `samsung_data.payment_credential.token_data.data` (`transformers.rs:2695-2699`) | `samsung_pay_data.payment_credential.token_data.data` (`transformers.rs:1665-1667`, `6659-6663`) |
| `token_data` proto/serde name | `#[serde(rename = "3_d_s")]` on `SamsungPayWalletCredentials.token_data` (ref `payment_method_data.rs:1027-1028`) | identical (`crates/types-traits/domain_types/src/payment_method_data.rs:963-964`) |
| `mpiData` | not emitted (Samsung Pay is not in the `matches!` guard, ref `transformers.rs:4074-4078`) | not emitted (`_ => None`, UCS `transformers.rs:2664`) |

Serialised payload, both sides:

```json
{"type":"samsungpay","samsungPayToken":"<opaque payload>"}
```

The committed field probe confirms the exact UCS output:

```
"paymentMethod":{"type":"samsungpay","samsungPayToken":"eyJhbGciOiJSUzI1NiIs…"}
```
(`data/field_probe/adyen.json`, `flows.authorize.SamsungPay.sample.body`)

> **VERDICT (B): byte-for-byte parity. No mismatch in the serialized JSON.**
> The only Samsung Pay finding is a capability-registry omission (DEV-08), not a wire defect.

---

## Dimension 1 — Authorization (Paze + Samsung Pay request construction)

**Samsung Pay** — full parity (see B). The UCS arm (`transformers.rs:1651-1670`) is a direct
transliteration of the reference arm (`transformers.rs:2695-2700`). It correctly leaves `mpiData`
unset, matching the reference's `matches!(wallet_data, Paze | ApplePay | GooglePay)` guard.

**Paze** — the `paymentMethod` object is wire-equivalent (see A). The surrounding request envelope
(`AdyenPaymentRequest`) is the pre-existing shared UCS wallet builder
(`transformers.rs:2571-2734`), which was not modified by this branch; its known pre-existing
deltas from the reference (`shopperName`/`countryCode` always `None`, no
`get_shopper_email(..., store_payment_method.is_some())` hard check, no `paymentdatasource`, no
`transactionLinkId`, no `store`/`splits`) apply equally to every wallet and are out of scope here.
`channel` is `None` on both sides, because the reference's `get_channel_type`
(`transformers.rs:2244-2251`) returns `Some(Channel::Web)` only for `GoPay`/`Vipps`.

**Blocking issue.** The Paze path is currently **unreachable at runtime** — see DEV-01. This is
not an opinion: the branch's own regenerated field probe records

```json
"flows": { "authorize": { "Paze": {
  "status": "not_supported",
  "error": "Invalid data format: payment_method. The provided payment method variant is empty or not supported by this flow" } } }
```

and `docs-generated/connectors/adyen.md:225` still shows `| Paze | x |` while Samsung Pay was
promoted `⚠ → ✓`. The error string comes verbatim from
`crates/types-traits/domain_types/src/types.rs:6815`.

## Dimension 2 — Recurring setup (SetupMandate / SetupRecurring)

**Structural difference (INTENTIONAL).** The reference has *no* dedicated SetupMandate request
builder: `ConnectorIntegration<SetupMandate,…>::get_request_body`
(reference `adyen.rs:520-538`) calls
`convert_setup_mandate_router_data_to_authorize_router_data` (reference `utils.rs:7718-7735`) and
then reuses `AdyenPaymentRequest::try_from(&AdyenRouterData<&PaymentsAuthorizeRouterData>)`. So the
reference's Samsung Pay / Paze SetupRecurring request is literally the Authorize request with
`amount = 0`. UCS instead has a dedicated `SetupMandateRequest` builder per payment-method family;
the new wallet impl (`transformers.rs:6605-6786`) is a faithful clone of the pre-existing card
SetupMandate impl (`transformers.rs:6431-6584`) with only `payment_method` and `mpi_data` swapped.
This is a legitimate UCS-architecture deviation.

| Aspect | Reference (via Authorize) | UCS wallet SetupMandate | Verdict |
|---|---|---|---|
| `storePaymentMethod` | `get_recurring_processing_model` → `is_mandate_payment()` when `setup_future_usage == OffSession` (ref `transformers.rs:2099-2108`) | `get_recurring_processing_model_for_setup_mandate` → `is_mandate_payment_for_setup_mandate` (UCS `transformers.rs:6967-7011`, `7064-7081`) | equivalent |
| `recurringProcessingModel` | `UnscheduledCardOnFile` for `OffSession` or `off_session == Some(true)`, else absent | identical logic | equivalent |
| `shopperInteraction` | `ContAuth` if `off_session`, else `Ecommerce`/`Moto` by `payment_channel` (ref `transformers.rs:2069-2082`) | `ContAuth` if `off_session`, else `Ecommerce` — no MOTO branch (UCS `transformers.rs:6959-6964`) | pre-existing gap (card path identical) |
| `shopperReference` | `get_connector_customer_id()`, hard error if absent in recurring branches | `connector_customer` else `"{merchant_id}_{customer_id}"` (UCS `transformers.rs:6698-6717`) | pre-existing UCS convention (card path identical) |
| zero-value auth | hard `minor_amount: MinorUnit::new(0)` | `request.amount.unwrap_or(0)` (UCS `transformers.rs:6923-6940`) | **DEV-06** (pre-existing, inherited) |
| mandate id extraction | `additional_data.recurring_detail_reference` → `MandateReference.connector_mandate_id` (ref `transformers.rs:4586-4595`) | identical, shared `get_adyen_response` (UCS `transformers.rs:4883-4892`) | parity |

The `shopper_reference` returned by `get_recurring_processing_model_for_setup_mandate` is
discarded (`let (_, _, _)` → third slot `_`) and a locally computed one is used instead. Both
produce the same string in the recurring branches, so this is dead-code noise rather than a bug —
and it is identical to the pre-existing card impl, so it is not introduced here.

**No wallet-specific recurring semantics were added.** Notably, `POST /storedPaymentMethods` is
not used (correctly — the reference does not use it either), and the `mandates` capability flag for
Paze is still `NotSupported` (DEV-09) even though SetupRecurring is now implemented for it.

## Dimension 3 — Request generation (field names, renames, nesting, optionality)

* `AdyenMpiData` is structurally identical on both sides — `#[serde_with::skip_serializing_none]`,
  `#[serde(rename_all = "camelCase")]`, same field set, same `dsTransID` / `threeDSVersion`
  explicit renames (ref `transformers.rs:402-418` vs UCS `transformers.rs:855-871`).
* `AdyenSamsungPay` / `SamsungPayPmData`: identical (see B).
* `AdyenNetworkTokenData` vs `AdyenPazeData`: field names identical; **UCS's struct is missing
  `#[serde_with::skip_serializing_none]`** (UCS `transformers.rs:182-193`) so `holderName` would be
  emitted as `null` rather than omitted when absent. Not triggerable on the Paze path (the
  `consumer.full_name` fallback makes it always `Some`), but a latent divergence on the generic
  NetworkToken PM path — DEV-10.
* Amount/currency: `Amount { currency, value }` on both sides; Authorize uses the converted minor
  amount, SetupMandate differs (DEV-06).
* Expiry: `expiryMonth` verbatim; `expiryYear` expanded to 4 digits in UCS only (DEV-04).
* Brand: `get_adyen_card_network` — UCS (`transformers.rs:1395-1413`) and reference
  (`transformers.rs:2481-2502`) map the same networks to the same `CardBrand` values;
  `Interac` → `None` → `brand` omitted in both. Co-badged brands are not modelled by either side.

## Dimension 4 — Response parsing

**Zero new response-parsing code was added by this branch.** Both wallets ride the shared
handlers. Verified equivalent:

| Handler | Reference | UCS |
|---|---|---|
| `Response` | `get_adyen_response` `transformers.rs:4528-4640` | `transformers.rs:4829-4940` |
| `RedirectionResponse` / `RedirectionErrorResponse` / `PresentToShopper` / `QrCodeResponse` / `WebhookResponse` | all dispatched | all dispatched (UCS `transformers.rs:6874-6898`) |
| mandate id | `additionalData.recurringDetailReference` → `MandateReference{connector_mandate_id}` | identical |
| network txn id | `additionalData.networkTxReference` | identical |
| `network_txn_link_id` | `additionalData.transactionLinkId` | identical |
| redirect/action | `get_redirection_response` builds `RedirectForm` | identical |

The UCS SetupMandate response impl (`transformers.rs:6856-6921`) handles all six
`AdyenPaymentResponse` variants and hard-codes `is_manual_capture = false`, which matches the
reference's SetupMandate handler passing `None` for capture method (reference `adyen.rs:556-582`).

No token/DPAN echo-back parsing exists on either side for Paze/Samsung Pay — parity.

## Dimension 5 — Status mapping

`get_adyen_payment_status` (UCS `transformers.rs:4427-4455` vs reference
`transformers.rs:480-523`) is a 1:1 port for every `resultCode`:

| Adyen | Reference | UCS |
|---|---|---|
| `AuthenticationFinished` | `AuthenticationSuccessful` | same |
| `AuthenticationNotRequired`, `Received` | `Pending` | same |
| `Authorised` | `Authorized` (manual) / `Charged` (auto) | same |
| `Cancelled` | `Voided` | same |
| `ChallengeShopper`, `RedirectShopper`, `PresentToShopper` | `AuthenticationPending` | same |
| `Error`, `Refused` | `Failure` | same |
| `Pending` | `AuthenticationPending` for `Pix`, else `Pending` | same |
| `Unknown` | returns `prev_status` | returns `AttemptStatus::Unspecified` (pre-existing UCS delta) |

**There is no wallet-specific status branching on either side**, so Paze and Samsung Pay map
exactly as cards do. No deviation introduced by this branch.

## Dimension 6 — Error handling

* Refusal reasons / `refusalReasonCode` / `refusalReasonRaw` / `merchantAdviceCode` splitting /
  `network_advice_code` / `network_decline_code` / `network_error_message`: identical logic
  (reference `transformers.rs:4537-4585` vs UCS `transformers.rs:4836-4882`). Untouched by this
  branch.
* New error paths introduced by the diff:
  * `IntegrationError::InvalidWalletToken { wallet_name: "Paze" }` when `CompleteResponse` fails to
    parse (UCS `transformers.rs:1467-1476`) — the reference has no equivalent branch. Copied
    verbatim from UCS Cybersource (`cybersource/transformers.rs:2478-2494`). INTENTIONAL.
  * `IntegrationError::MissingRequiredField { field_name: "paze_decrypted_data.dynamic_data.dynamic_data_value" }`
    (UCS `transformers.rs:1500-1506`) — **a failure mode the reference does not have** (DEV-02b).
  * `IntegrationError::NotImplemented("payment method")` for non-Paze/non-Samsung wallets in
    SetupMandate (UCS `transformers.rs:6672-6678`) — reasonable, mirrors the card-path style.
* The reference's Paze `NotImplemented` arm passes the literal string `"Cybersource"`
  (reference `transformers.rs:2718-2721`) — a reference bug. UCS does not reproduce it because the
  UCS architecture has no `payment_method_token` absent-case. INTENTIONAL improvement.

## Dimension 7 — Metadata handling (mpiData / TAVV / ECI / additionalData / browser info / shopper data)

### mpiData — Paze

| Field | Reference (`transformers.rs:4079-4092`) | UCS (`build_paze_mpi_data`, `transformers.rs:1539-1559`) |
|---|---|---|
| `directoryResponse` | `TransactionStatus::Success` | `TransactionStatus::Success` |
| `authenticationResponse` | `TransactionStatus::Success` | `TransactionStatus::Success` |
| `cavv` | `None` | `None` |
| `tokenAuthenticationVerificationValue` | **`paze_data.token.payment_account_reference`** | **`dynamic_data[…CRYPTOGRAM_3DS…].dynamic_data_value`** ← DEV-02 |
| `eci` | `paze_data.eci.clone()` (omitted when absent) | `paze.eci` **or literal `"05"`** ← DEV-03 |
| `dsTransID`, `threeDSVersion`, `challengeCancel`, `riskScore`, `cavvAlgorithm` | all `None` | all `None` |

The `payment_account_reference` convention is **the only** Paze-cryptogram convention that exists
in the reference codebase — `PazeDecryptedData.dynamic_data` is never read by any reference
connector, and `payment_account_reference` is used identically by reference Adyen
(`transformers.rs:4084`) and reference Cybersource (`transformers.rs:2022`). Critically, **UCS's
own Cybersource connector also uses `paze_data.token.payment_account_reference`**
(`crates/integrations/connector-integration/src/connectors/cybersource/transformers.rs:1723`), so
the new Adyen code diverges from UCS's own established Paze convention as well.

### networkToken vs native payment method type
Correct on both sides — Paze → `networkToken`, Samsung Pay → native `samsungpay`. See A and B.

### additionalData
Shared UCS builder (`get_additional_data` `transformers.rs:6048-6096`,
`get_additional_data_for_setup_mandate` `transformers.rs:7013-…`). Wallet-agnostic; no changes in
this branch. Pre-existing deltas vs reference (`paymentdatasource` for cryptogram-less Google Pay,
`transactionLinkId` from NTI mandate refs) are unrelated to Paze/Samsung Pay.

### Browser info
`get_browser_info` / `get_browser_info_for_setup_mandate` (UCS `transformers.rs:7938-7970`) apply
the identical predicate to the reference (`transformers.rs:2124-2145`): 3DS **or** Card **or**
BankRedirect **or** GoPay **or** GooglePay. Paze and Samsung Pay are `PaymentMethod::Wallet` with
`NoThreeDs`, so `browserInfo` is omitted on both sides. Parity.

### Shopper data
Authorize wallet path: `shopperName: None`, `countryCode: None`, `channel: None` on **both** sides.
Parity. SetupMandate wallet path: UCS populates `shopperName` and `countryCode` (inherited from the
card SetupMandate clone) whereas the reference's SetupMandate is the wallet-Authorize path and
therefore sends neither — DEV-11 (low, arguably an improvement, internally consistent with UCS's
card path).

## Dimension 8 — Edge cases

| Edge case | Reference | UCS | Note |
|---|---|---|---|
| `payment_method_token` absent for Paze | `NotImplemented` (`transformers.rs:2718`) | N/A — Paze payload travels inside `WalletData::Paze` because `PaymentFlowData.payment_method_token` was deliberately removed in UCS (`domain_types/src/router_data.rs:3838-3840` "Dead code: nothing populates this…") | INTENTIONAL |
| Decrypted vs `CompleteResponse` | reference has a single decrypted variant (core decrypts) | `PazeWalletData::{Decrypted, CompleteResponse}`; `CompleteResponse` is parsed as **plaintext JSON** of `PazeDecryptedData` (`transformers.rs:1466-1476`) | DEV-07 — copied from UCS Cybersource including its `// TODO: This needs to be tested` |
| `eci` absent | field omitted from `mpiData` | literal `"05"` injected | DEV-03 |
| `dynamic_data` empty | never read | **hard error**, whole authorization fails | DEV-02b |
| Expiry year 2-digit | sent as-is | expanded to 4 digits | DEV-04 (safe) |
| `holderName` all sources absent | omitted | falls back to `consumer.full_name` (never `None`) | DEV-05 |
| Co-badged brand | not modelled | not modelled | parity |
| `Interac` card network | `brand` → `None` → omitted | same | parity |
| Samsung Pay + `mpiData` | never emitted | never emitted | parity |
| Non-Paze/Samsung wallet in SetupRecurring | reference would attempt the real wallet | `NotImplemented("payment method")` | acceptable |

---

## Deviation table

| ID | Dimension | Deviation | Class | Sev | UCS location | Failing scenario |
|---|---|---|---|---|---|---|
| DEV-01 | 1, 8 | proto `PazeSdk` has **no arm** in `ForeignTryFrom<grpc PaymentMethod> for common_enums::PaymentMethod`, so every Paze request hits the catch-all and is rejected before reaching the Adyen transformer | **DEFECT** | **HIGH** | `crates/types-traits/domain_types/src/types.rs:6330` (impl) — catch-all at `types.rs:6806-6819`; `SamsungPaySdk` arm exists at `types.rs:6388-6391`, `PazeSdk` only exists at `types.rs:1718` and `types.rs:2675` | Any `PaymentService.Authorize` or `SetupRecurring` with `payment_method.paze_sdk` returns `InvalidDataFormat{field_name:"payment_method"}` — "The provided payment method variant is empty or not supported by this flow". Reproduced by the branch's own probe: `data/field_probe/adyen.json` → `flows.authorize.Paze.status = "not_supported"`, and `docs-generated/connectors/adyen.md:225` still `\| Paze \| x \|`. **The entire Paze feature (both flows) is dead code.** |
| DEV-02 | 7 | `mpiData.tokenAuthenticationVerificationValue` sourced from `dynamic_data[…CRYPTOGRAM_3DS…].dynamic_data_value` instead of `token.payment_account_reference` | **DEFECT** | **HIGH** | `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1479-1507` (`get_paze_token_cryptogram`), consumed at `transformers.rs:1545-1547` | A Paze authorization that succeeds on the reference (and on UCS Cybersource, which uses `payment_account_reference` at `cybersource/transformers.rs:1723`) sends a **different TAVV value** to Adyen. `PazeDecryptedData.dynamic_data` is read by **no** connector in the reference tree. Same wallet payload ⇒ different `mpiData` ⇒ different Adyen authentication outcome/liability. |
| DEV-02b | 6, 8 | Hard `MissingRequiredField` when `dynamic_data` is empty or every entry has `dynamic_data_value == None` | **DEFECT** | **HIGH** | `transformers.rs:1500-1506` | `PazeDynamicData` is `repeated` + all-`optional` in the proto (`crates/types-traits/grpc-api-types/proto/payment_methods.proto:526-530, 563`). A caller that populates `PazeDecryptedData` without `dynamic_data` (perfectly valid per proto, and sufficient for the reference and for UCS Cybersource) gets a hard 4xx instead of a payment. New failure mode with no reference counterpart. |
| DEV-03 | 7, 8 | `mpiData.eci` defaults to the literal `"05"` when the Paze payload carries no ECI | **DEFECT** | **MED** | `transformers.rs:124` (`PAZE_DEFAULT_ECI`), applied at `transformers.rs:1548-1553` | Reference omits `eci` entirely when absent (`eci: paze_data.eci.clone()` + `skip_serializing_none`). UCS asserts ECI `05` = "fully authenticated e-commerce" together with `directoryResponse: Success` / `authenticationResponse: Success`. For a Paze payload that carried no ECI this fabricates authentication metadata and can shift chargeback liability on a transaction that was never authenticated. |
| DEV-04 | 3, 8 | `expiryYear` passed through `expand_expiry_year_to_four_digits` | INTENTIONAL | LOW | `transformers.rs:1519-1521` | No-op when Paze returns a 4-digit year (the normal case); strictly safer if a 2-digit year ever arrives, since Adyen requires 4 digits. Divergence from reference is benign. |
| DEV-05 | 3 | `holderName` gains a third fallback to `consumer.full_name` | INTENTIONAL | LOW | `transformers.rs:1522-1528` | Reference would omit `holderName` when both billing sources are empty; UCS always sends one. Slightly different payload; low risk. Note `consumer.full_name` is the Paze *account* holder, not necessarily the cardholder. |
| DEV-06 | 2 | SetupRecurring does not force a zero-value authorization: `request.amount.unwrap_or(0)` | DEFECT (pre-existing, inherited) | MED | `transformers.rs:6923-6940` (`get_amount_data_for_setup_mandate`) | Reference hard-codes `minor_amount: MinorUnit::new(0)` (`hyperswitch5 crates/hyperswitch_connectors/src/utils.rs:7732-7734`). A `SetupRecurring` call carrying a non-zero `amount` produces a **real charge** on Adyen instead of a zero-value verification. Shared with the pre-existing card SetupMandate path — **not introduced by this branch**, but the new wallet path inherits it, and Paze/Samsung Pay make it more likely to be exercised. |
| DEV-07 | 8 | `PazeWalletData::CompleteResponse` deserialised as plaintext JSON of `PazeDecryptedData` | DEFECT (pre-existing pattern) | MED | `transformers.rs:1464-1477` | Copied verbatim (including the semantics) from UCS Cybersource `cybersource/transformers.rs:2478-2494`, which carries `// TODO: This needs to be tested`. If `complete_response` is the Paze SDK's encrypted/JWE blob rather than decrypted JSON, every such request fails with `InvalidWalletToken`. The reference never sees this variant at connector level (decryption happens in core), so there is nothing to compare against — but the code is unverified and is duplicated rather than centralised. |
| DEV-08 | 8 | Samsung Pay is **not** registered in `ADYEN_SUPPORTED_PAYMENT_METHODS` | DEFECT | MED | `crates/integrations/connector-integration/src/connectors/adyen.rs:1272-1406` (only `PaymentMethodType::Paze` was added, at `adyen.rs:1394-1405`) | Reference registers both (`hyperswitch5 adyen.rs:2986-3006`). Requests still succeed because an unknown PMT falls through `.unwrap_or(true)` in `validate_connector_against_payment_request` (`crates/types-traits/interfaces/src/connector_types.rs:713-732`), so this is capability-metadata only: Samsung Pay's supported capture methods / refund / mandate capabilities are unreported to callers and to generated docs. |
| DEV-09 | 2 | Paze registered with `mandates: FeatureStatus::NotSupported` although SetupRecurring for Paze was implemented in this same branch | DEFECT | MED | `crates/integrations/connector-integration/src/connectors/adyen.rs:1398` | Self-contradictory capability metadata: `9e3c4de32` implements `SetupMandate` for Paze while the registry advertises no mandate support. (The reference also says `NotSupported`, but the reference never claimed to implement it.) Callers doing capability-driven routing will never route a Paze mandate here. |
| DEV-10 | 3 | `AdyenNetworkTokenData` lacks `#[serde_with::skip_serializing_none]`, so `holderName: null` is emitted when `None` | DEFECT (pre-existing) | LOW | `transformers.rs:182-193` | Reference `AdyenPazeData` uses `skip_serializing_none` and omits the key. Not reachable on the Paze path (DEV-05 guarantees `Some`), but the generic `PaymentMethodData::NetworkToken` path (`transformers.rs:1450-1459`, which sets `holder_name: card_holder_name` and can be `None`) sends `"holderName": null` where the reference sends nothing. |
| DEV-11 | 7 | Wallet SetupRecurring sends `shopperName` and `countryCode`; reference's SetupMandate (= wallet Authorize) sends neither | INTENTIONAL | LOW | `transformers.rs:6741-6748`, `6758-6760` | Consistent with UCS's own card SetupMandate impl; extra shopper data is harmless to Adyen. |
| DEV-12 | 2 | `get_recurring_processing_model_for_setup_mandate` errors `MissingRequiredField{"customer_id"}` **before** the match, i.e. even for the non-recurring branch | DEFECT (pre-existing) | LOW | `transformers.rs:6977-6984` | Reference only requires the shopper reference inside the recurring branches. Shared with the card path; the new wallet path inherits it. |
| DEV-13 | 5 | `AdyenStatus::Unknown` → `AttemptStatus::Unspecified` instead of retaining `prev_status` | INTENTIONAL (pre-existing, UCS has no `prev_status` in scope) | LOW | `transformers.rs:4451-4453` | Documented in-code; UCS signals the core to retain instead of retaining locally. |
| DEV-14 | — | Generated artefacts (`data/field_probe/adyen.json`, `docs-generated/**`) were regenerated at `8b8d53fbc`, which precedes both SetupRecurring commits | DEFECT (process) | LOW | `data/field_probe/adyen.json`, `docs-generated/connectors/adyen.md` | `setup_recurring` in the probe contains only the card `default` entry; the new Paze/Samsung Pay SetupRecurring paths have **never been probed**. The two SetupRecurring commits are also still labelled `wip … [FAILED]`. |

---

## Summary of defects by severity

* **HIGH (3):** DEV-01, DEV-02, DEV-02b — all on the Paze path.
* **MED (4):** DEV-03, DEV-06 (pre-existing, inherited), DEV-07 (pre-existing pattern), DEV-08, DEV-09.
* **LOW (3):** DEV-10 (pre-existing), DEV-12 (pre-existing), DEV-14.

Newly introduced by this branch: DEV-01 (by omission), DEV-02, DEV-02b, DEV-03, DEV-08, DEV-09, DEV-14.

## Recommended fixes

1. **DEV-01** — add to `crates/types-traits/domain_types/src/types.rs` (impl at line 6330), next to
   the `SamsungPaySdk` arm at 6388-6391:
   ```rust
   grpc_api_types::payments::PaymentMethod {
       payment_method: Some(grpc_api_types::payments::payment_method::PaymentMethod::PazeSdk(_)),
   } => Ok(Self::Wallet),
   ```
   Then regenerate the field probe and confirm `flows.authorize.Paze.status == "supported"`.
2. **DEV-02 / DEV-02b** — replace `get_paze_token_cryptogram` with
   `paze_decrypted_data.token.payment_account_reference.clone()`, matching reference Adyen,
   reference Cybersource and UCS Cybersource. Delete the now-dead helper and its error path. If the
   TAVV-vs-PAR question is genuinely a reference bug worth fixing, fix it **centrally and
   consistently** (all Paze connectors in both trees) rather than diverging in one connector.
3. **DEV-03** — drop `PAZE_DEFAULT_ECI`; emit `eci: paze_decrypted_data.eci.clone()` so the key is
   omitted when the wallet supplied no ECI.
4. **DEV-08 / DEV-09** — register `PaymentMethodType::SamsungPay` in
   `ADYEN_SUPPORTED_PAYMENT_METHODS`, and set `mandates: FeatureStatus::Supported` for the wallets
   whose SetupRecurring path was actually implemented.
5. **DEV-14** — regenerate `data/field_probe/adyen.json` and `docs-generated/**` at HEAD, add
   Paze/Samsung Pay probes for `setup_recurring`, and re-title the two `wip … [FAILED]` commits.

---

## VERDICT

**Samsung Pay: FULL PARITY (Authorize and SetupRecurring) — byte-identical wire payload, correct native `samsungpay` type, correct `samsungPayToken` field, correct source field, correctly no `mpiData`; the only issue is a missing capability-registry entry. Paze: NOT AT PARITY AND NON-FUNCTIONAL — the `paymentMethod` object is wire-equivalent to the reference's `AdyenPaze` (both serialize `type: "networkToken"`, so claim A is vindicated), but the feature is unreachable because proto `PazeSdk` has no arm in the gRPC→`PaymentMethod` mapping (proven by the branch's own field probe), and once reachable it would still send a different `mpiData.tokenAuthenticationVerificationValue` than the reference and hard-fail on payloads the reference accepts. 3 HIGH / 4 MED / 3 LOW defects. NOT MERGE-READY.**
