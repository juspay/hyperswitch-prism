# Adyen Paze / SamsungPay — PR-Review Theme Audit

**Repo:** `juspay/hyperswitch-prism` · **Branch:** `feat/adyen_grace`
**Commits under review:** `8a6d97cc5`, `9aac731f7`, `9e3c4de32`, `1b1c832d9` (base `45351c251`)
**Files changed:** `crates/integrations/connector-integration/src/connectors/adyen.rs` (+12),
`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs` (+363/−7)

Review corpus: inline review comments, review bodies and issue comments from the **last 20 merged PRs**
(2027, 2025, 2024, 2022, 2013, 2012, 2011, 2009, 2007, 2006, 2005, 2004, 2003, 2000, 1998, 1997, 1996,
1994, 1993, 1992). Comment density in that window is low (only 5 of 20 carry substantive human review),
so the corpus was **widened to the last 90 merged PRs** to establish which findings are genuinely
*recurring* rather than one-off. PR numbers below are drawn from that widened set; bot/CI noise and
GRACE-template boilerplate were excluded.

---

## PART 1 — RECURRING REVIEW THEMES

| # | Theme | Distinct PRs | Representative quotes | Detection rule (Rust, this codebase) |
|---|---|---|---|---|
| **T1** | **Silent error swallowing / lossy fallback** — `.ok()`, `unwrap_or_default()`, `unwrap_or_else(default)` on a field the connector actually requires | **8** — 1788, 1811, 1817, 1855, 1910, 1942, 1944, 2003 | *"**[S1 blocker]** `.ok()` swallows real errors and silently corrupts Level III detection … a real build error → `product_details = None` → the transaction **silently downgrades from Level III to Level II** with zero diagnostics"* (PR1942) · *"Should we avoid `unwrap_or_default()` for card number here? … Missing card number should probably fail conversion with `MissingRequiredField` instead of becoming `\"\"`"* (PR1788) · *"When `return_url` is absent or not HTTPS, `callback_url` silently becomes `https://www.google.com` … with no error raised anywhere"* (PR1944) · *"`unwrap_or_default()` makes this the literal `\"refresh_\"` for every such call, so the reference id collapses to a constant"* (PR2003) | `.ok()` / `.unwrap_or(` / `.unwrap_or_default()` / `.unwrap_or_else(\|_\| …)` applied to a `Result`/`Option` that feeds a **request** field the remote API treats as required. Also `.and_then(Result::ok)`. |
| **T2** | **Hardcoded magic values** that should be a named constant, derived from data, or come from config | **9** — 1777, 1806, 1811, 1855, 1873, 1910, 1944, 1952, 2004 | *"Why are these statuses hardcoded to pending?"* (PR1777) · *"why hard coded `Hyperswitch`?"* (PR1806) · *"why is this hardcoded?"* (PR2004) · *"why account kind is hardcoded — how are you deciding is it threeds or not"* (PR1952) · *"avoid magic strings"* (PR1910) · *"This hardcodes `dev_payments_queue` … if `base_url` ever changes, this step will silently create the wrong topic"* (PR1855) | Struct-literal fields assigned a literal (`"05"`, `Status::Success`, `Sequence::Initial`) that the reviewer would expect to be derived from the request/response. Bare string/enum literals in request builders. |
| **T3** | **Wrong / missing enum arm; catch-all `_ =>` hiding new variants; inconsistent mapping** | **7** — 1777, 1779, 1873, 1910, 1952, 2004, 2005 | *"**[S2] Razorpay silently mis-maps `Skrill` to `RazorpayWalletType::EaseBuzz`** … Razorpay alone would send a Skrill intent to Razorpay **as an EaseBuzz wallet** instead of failing … a latent payment-correctness bug"* (PR1779) · *"Because `#[serde(other)] Unknown` is the catch-all, this fails **silently** … the real reason is lost"* (PR1873) · *"Inner match is exhaustive (no `_`), so a new variant still breaks compilation here"* (PR1788, approved shape) · *"is it fine we are removing the status mapping, was it dead code in the first place"* (PR2004) | `match` over a domain enum ending in `_ => …` where sibling matches in the same file are exhaustive; a new enum value folded into an existing `\|`-group whose semantics differ; a `PaymentMethodType`/`WalletData` variant handled in one flow but absent from the mapping another flow needs. |
| **T4** | **Eroded error diagnostics** — `context: Default::default()`, no `attach_printable`, unactionable / wrong messages, dropped code/message/reason | **9** — 1778, 1779, 1806, 1812, 1873, 1910, 1944, 2003, 2007 | *"`code`/`message` are hardcoded to `NO_ERROR_CODE`/`NO_ERROR_MESSAGE` … even a well-formed decline reaches the merchant with zero diagnostic information"* (PR1873) · *"**[S3] Residual `context: Default::default()` in new error sites** … drops diagnostic context"* (PR1779) · *"try to use `attach_printable` with `change_context` so that while debugging things are clear"* (PR1778) · *"The `MissingRequiredField` message right above it is good and already enumerates the supported banks; the `NotImplemented` arm should say something equivalent"* (PR1812) · *"the Apple Pay `InvalidWalletToken` error … lists Google Pay fields … This misleads integrators"* (PR2007, raised 5×) | `IntegrationError::* { context: Default::default() }` in **newly added** error sites; `?`/`change_context` with no `.attach_printable`; `NotImplemented("payment method")`-style messages that name neither connector nor method. |
| **T5** | **Duplicated code that should be a shared helper** | **6** — 1812, 1910, 1942, 1980, 2003, 1944 | *"the `Card` arm plus the entire `BankRedirect` block was duplicated **verbatim** between `AirwallexPaymentRequest` and `AirwallexConfirmRequest`, and the card arm a third time in the SetupMandate builder"* (PR1812) · *"when I extracted the block, all three were **byte-for-byte identical**, which is exactly the drift risk you're describing"* (PR1980) · *"`TesouroApiErrorData` and `TesouroGraphQlError` describe the same wire shape … collapsing them into one type is the obvious follow-up"* (PR1910) · *"Moving the change out would mean … duplicating the handler"* (PR2003) | Two `TryFrom` impls in the same file whose bodies differ only in one or two fields; the same struct-literal construction inlined at ≥2 call sites when a helper already exists for a sibling case. |
| **T6** | **Capability-surface drift** — `SupportedPaymentMethods`, `data/field_probe/*.json`, `docs-generated/*` out of sync with what the code now does | **5** — 1779, 1811, 1812, 1873, 1944 | *"Please confirm … dropping `proxy_setup_recurring` from the advertised flows is intended — right now it's an **undocumented capability-surface regression**"* (PR1811) · *"I would want to land the proto half first so this one is never in a state where Klarna is **advertised and broken**"* (PR1812) · *"Reachability is gated by Razorpay **not advertising** Skrill support"* (PR1779) · *"`data/field_probe/glomopay.json` has no `customer_get` key, and `docs-generated/all_connector.md` consequently reports `Customer.Get = x`"* (PR1944) | A new payment method / flow implemented in `transformers.rs` without a matching `add(PaymentMethod::…, PaymentMethodType::…, PaymentMethodDetails { … })` entry, or with `FeatureStatus` that contradicts the implemented flows; `data/field_probe/<connector>.json` untouched by a PR that changes what the connector accepts. |
| **T7** | **Secrets / PII typing** — fields that should be `Secret<T>`, or exposed before masking | **5** — 1788, 1820, 1944, 1986, 2004 | *"should these be secret?"* (PR2004, raised 4× across 3 reviewers) · *"all these data should be `Secret` right"* (PR1788) · *"You first exposed the PII data then converted it to secret"* (PR1944) · *"`token` is now `Secret<String>` … Dropped the hand-rolled `.peek().parse()`"* (PR1986) | Plain `String`/`u8` fields on request/response structs holding tokens, URLs with embedded credentials, names, addresses, phone, geo. `.expose()`/`.peek()` whose result is stored, logged, or re-wrapped rather than immediately consumed. |
| **T8** | **Panics / unvalidated parsing of untrusted connector data** | **4** — 1811, 1812, 1953, 2003 | *"`raw.len() != 4` is a byte-length check, but `split_at(2)` needs a char boundary … `\"aé1\"` … **panics** … takes down the request inside the tonic handler"* (PR2003) · *"The year half isn't validated at all. `expand_expiry_year_to_four_digits` just prefixes the century for any 2-char string with no digit check, so `\"12ab\"` comes back as … `card_exp_year = \"20ab\"`"* (PR2003) · *"this interpolates the raw `card_exp_month.peek()` without 2-digit padding, so a January expiry renders as `2031-1`"* (PR1953) | `unwrap()`/`expect()`/`panic!`/`[i]`/`split_at`/unchecked arithmetic on connector-supplied data; card/expiry/token fields forwarded to the wire without `CardExpirationMonth::try_from`, ASCII-digit guards, or an equivalent validator. |
| **T9** | **Amount handling bypassing the framework converter** | **2** — 1788, 1789 | *"Can we use `MinorUnit` for amount framework"* (PR1788) · *"Recurring amounts here serialize as raw minor-unit `i64` strings … while the order/transaction amount goes through `KountAmountConvertor::convert` … the two formatting paths should be reconciled … to avoid future unit drift"* (PR1789) | `.get_amount_as_i64().to_string()` / hand-rolled float or string amount formatting where the connector has an `AmountConvertor`; two amount fields in one request built by different paths. |
| **T10** | **Unnecessary clones where a borrow would do** | **3** — 1779, 1788, 1812 | *"Borrowing `payment_method_data` instead of matching on `.clone()` also drops two clones per request"* (PR1812) · *"can you optimize this"* (PR1779) · *"here code for proxy and non-proxy flow are exactly same except the generic type"* (PR1788) | `match x.clone() { … }` / `*boxed.clone()` where `&x` suffices; a `Box<T>` deref-cloned back into an owned `T`; the same expensive derivation recomputed at two call sites in one request build. |
| **T11** | **Dead / unreachable code shipping** | **8** — 1811, 1812, 1855, 1910, 1944, 1952, 2003, 2013 | *"It leaves `PaypalOrderAuthorizeRequest` (struct + ~138-line `TryFrom`) fully **dead** — only `pub` suppresses the warning, so an orphan ships"* (PR1811) · *"That `Card` arm is actually **unreachable** … replaced with an explicit `NotImplemented`"* (PR1952) · *"remove test file"* (PR2003) · *"if not necessary you can remove this as we don't have any pipeline or workflow to test this"* (PR2013) | `pub` items with no in-repo caller; match arms proven unreachable by an upstream guard; `alias`/variant/const that nothing reads. |
| **T12** | **Missing serialization/behaviour coverage for new wire logic** (in tension with T11 — reviewers also delete inline `mod tests`) | **4** — 1812, 1944, 1994, 2006 | *"**[S2] no automated coverage for the 1048 added lines** … pure serialization logic that unit-tests cheaply and will otherwise drift silently"* (PR1812) · *"**[S1]** Refund and RSync suites assert against a hardcoded stub, so they cannot fail"* (PR1944) · *"there is no connector spec or serialized-body test covering the new `data` field behavior"* (PR2006) · *"Please add a behavior-level serialization test proving that …"* (PR1994) | New request/response structs or untagged enums with no wire-shape assertion anywhere; the convention for connectors that do test is a separate `connectors/<name>/test.rs`, not inline `mod tests` (PR2013). |

---

## PART 2 — AUDIT OF THE NEW ADYEN CODE, THEME BY THEME

Line numbers are **current working-tree lines** in
`crates/integrations/connector-integration/src/connectors/adyen/transformers.rs` (`transformers.rs`)
and `crates/integrations/connector-integration/src/connectors/adyen.rs` (`adyen.rs`).

### Summary of findings

| # | File:line | Theme | Severity |
|---|---|---|---|
| F1 | `adyen.rs:1399` | T6 | **HIGH** |
| F2 | `transformers.rs:1235-1239` (reachable via `:7137`) | T3 | **HIGH** |
| F3 | `adyen.rs:1394-1404` (absence) | T6 | **HIGH** |
| F4 | `transformers.rs:1492-1497` | T1 | **MED** |
| F5 | `transformers.rs:124`, `:1547-1552` | T2 | **MED** |
| F6 | `transformers.rs:1541-1542` | T2 | **MED** (pattern PRE-EXISTING) |
| F7 | `transformers.rs:1519-1522` | T8 | **MED** |
| F8 | `transformers.rs:6605-6787` | T5 | **MED** |
| F9 | `transformers.rs:1665-1667` + `:6659-6665` | T5 | **LOW** |
| F10 | `transformers.rs:1466`, `:1692` + `:2661` | T10 | **MED** |
| F11 | `transformers.rs:1470-1473`, `:1500-1503` | T4 | **MED** |
| F12 | `transformers.rs:6669-6675` | T3 | **MED** |
| F13 | `transformers.rs:6670-6673` | T4 | **LOW** |
| F14 | `data/field_probe/adyen.json` (not updated) | T6 | **MED** |
| F15 | `transformers.rs:1529` | T1 | **LOW** (mirrors PRE-EXISTING card path) |

---

### T1 — Silent error swallowing / lossy fallback *(PRs 1788, 1811, 1817, 1855, 1910, 1942, 1944, 2003)*

#### F4 — [MED] Cryptogram fallback silently sends the **wrong** dynamic-data value as the TAVV

`transformers.rs:1479-1506`, offending block at **1492-1497**:

```rust
        .find(|dynamic_data| {
            dynamic_data.dynamic_data_value.is_some()
                && dynamic_data.dynamic_data_type.as_deref()
                    .is_some_and(|data_type| data_type.eq_ignore_ascii_case("CRYPTOGRAM_3DS"))
        })
        .or_else(|| {
            paze_decrypted_data
                .dynamic_data
                .iter()
                .find(|dynamic_data| dynamic_data.dynamic_data_value.is_some())   // <-- any type
        })
```

`PazeDynamicData.dynamic_data_type` is `Option<String>` (`domain_types/src/router_data.rs:3832-3836`)
and Paze's `dynamicData` array is not restricted to `CRYPTOGRAM_3DS` — a dynamic card security code
(`DYNAMIC_CARD_SECURITY_CODE`) is the other value the schema carries. When no `CRYPTOGRAM_3DS` entry
is present, this fallback picks whatever is first and ships it to Adyen as
`mpiData.tokenAuthenticationVerificationValue`. Adyen returns a generic cryptogram-mismatch refusal
and the true cause — "the payload had no TAVV" — is invisible. This is exactly the PR1942 shape
(*"a real build error → … silently downgrades … with zero diagnostics"*) and the PR1944 shape
(*"silently becomes `https://www.google.com` … with no error raised anywhere"*).

**Fix:** delete the `.or_else(…)` arm. Require `CRYPTOGRAM_3DS` and, on absence, return the existing
`MissingRequiredField` with `.attach_printable(format!("Paze dynamicData carried no CRYPTOGRAM_3DS; types present: {:?}", types))`.

#### F15 — [LOW] Unmappable card network silently drops `brand`

`transformers.rs:1529`:

```rust
        brand: get_adyen_card_network(paze_decrypted_data.payment_card_network.clone()),
```

`get_adyen_card_network` (`transformers.rs:1395-1413`) returns `None` for
`CardNetwork::Interac`, and `AdyenNetworkTokenData.brand` is `#[serde(skip_serializing_if = "Option::is_none")]`
(`transformers.rs:189-190`), so an unmappable network is dropped rather than rejected.
Practical risk is low (Paze is Visa/MC/Amex/Discover) and this **mirrors the pre-existing card path**
at `transformers.rs:1425-1436`, which also treats `brand` as optional. Noted for completeness; no
change required unless Adyen rejects a Paze `networkToken` without `brand`.

**Otherwise clean for T1:** there is no `.unwrap_or_default()`, no hardcoded `""`, and no
`.parse::<T>().unwrap_or(0)` anywhere in the diff. `get_address_info(..).and_then(Result::ok)`
appears at `transformers.rs:6698` and `:6741` in the new wallet impl, which *is* a T1 pattern — but
it is a **verbatim copy of the pre-existing card `SetupMandate` impl** (`:6491`, `:6555`) and of the
Authorize wallet path (`:2692`). **PRE-EXISTING**; flagged only as part of F8 (duplication), not as a
new T1 violation.

---

### T2 — Hardcoded magic values *(PRs 1777, 1806, 1811, 1855, 1873, 1910, 1944, 1952, 2004)*

#### F5 — [MED] `PAZE_DEFAULT_ECI = "05"` masks a missing liability-shift indicator, and disagrees with the sibling network-token path

`transformers.rs:122-124`:

```rust
/// `mpiData.eci` is mandatory for Adyen network-token authorizations, but the decrypted Paze
/// payload does not always carry one. Fall back to the fully-authenticated e-commerce indicator.
const PAZE_DEFAULT_ECI: &str = "05";
```

used at `transformers.rs:1547-1552`:

```rust
        eci: Some(
            paze_decrypted_data.eci.clone()
                .unwrap_or_else(|| PAZE_DEFAULT_ECI.to_string()),
        ),
```

Two problems:

1. **It asserts an authentication outcome the payload did not supply.** ECI `05` is *fully
   authenticated, liability shifted*. Defaulting to it whenever Paze omits `eci` claims liability
   shift on transactions where the authentication state is unknown. This is the T2 shape reviewers
   pushed back on in PR1777 (*"Why are these statuses hardcoded to pending?"*) and PR1952
   (*"the hardcoded `CardThreeDs` did read like a runtime 3DS decision, which it isn't"*).
2. **It contradicts the other Adyen network-token path in the same file.** The self-managed
   network-token builder at `transformers.rs:3475-3486` hardcodes `eci: Some("02".to_string())` for
   the structurally identical `mpiData` block. Adyen now receives two different default ECIs from two
   Adyen network-token paths, and neither is a named shared constant. This is the PR1789 divergence
   complaint applied to ECI rather than amounts.

**Fix:** prefer returning `MissingRequiredField { field_name: "paze_decrypted_data.eci" }` (consistent
with how the cryptogram is treated one field above). If a default must exist, hoist a single
`ADYEN_NETWORK_TOKEN_DEFAULT_ECI` constant used by both `build_paze_mpi_data` and the `:3475` builder,
and document which Adyen doc mandates the value.

#### F6 — [MED, pattern PRE-EXISTING] `directoryResponse` / `authenticationResponse` unconditionally `Success`

`transformers.rs:1540-1542`:

```rust
    Ok(AdyenMpiData {
        directory_response: common_enums::TransactionStatus::Success,
        authentication_response: common_enums::TransactionStatus::Success,
```

Hardcoded regardless of what the Paze payload carries. **The identical two lines are pre-existing**
at `transformers.rs:3476-3477` (network token), `:2620-2621` (Apple Pay) and `:2642-2643`
(Google Pay), so this follows established Adyen precedent and is *not* a regression. It is flagged
because the new code is the first place these are asserted for a wallet whose payload does carry
authentication signalling (`eci`, `dynamic_data_type`), which is where F5 bites. Fix alongside F5 or
add a comment at `:1541` recording that Adyen requires `Y/Y` for self-managed network tokens
regardless of payload — reviewers in PR1812 asked for exactly that kind of note
(*"silently ignoring a connector-supplied `method` deserves a note at the field itself so it is not
'fixed' later by someone who has not read this comment"*).

---

### T3 — Wrong / missing enum arm; catch-all `_ =>` *(PRs 1777, 1779, 1873, 1910, 1952, 2004, 2005)*

#### F2 — [HIGH] Paze mandates can be created but never charged — `PaymentType::try_from` has no `Paze` arm

`transformers.rs:1200-1240`, the offending fallthrough at **1235-1239**:

```rust
            common_enums::PaymentMethodType::PaySafeCard => Ok(Self::PaySafeCard),
            _ => Err(IntegrationError::NotImplemented(
                utils::get_unimplemented_payment_method_error_message("Adyen"),
                Default::default(),
            )
            .into()),
```

This mapping is consumed by the **RepeatPayment (MIT)** builder at `transformers.rs:7133-7138`:

```rust
                let adyen_mandate = AdyenMandate {
                    payment_type: match payment_method_type {
                        Some(pm_type) => PaymentType::try_from(&pm_type)?,
                        None => PaymentType::Scheme,
                    },
```

`PaymentMethodType::SamsungPay` **is** in the `=> Ok(Self::Scheme)` group (`transformers.rs:1222`),
so a Samsung Pay mandate can be charged. `PaymentMethodType::Paze` is **absent**, so it falls to
`_ => NotImplemented`. Net effect of these commits: `SetupRecurring` now *creates* a Paze mandate at
Adyen, but every subsequent `RepeatPayment` against that `connector_mandate_id` fails with a bare
"payment method not implemented". The mandate is orphaned connector-side.

The `PaymentType::try_from` function itself is **PRE-EXISTING and untouched by this diff** — but the
gap is *newly reachable* only because these commits made Paze mandates creatable. This is precisely
PR1779's finding shape (*"a latent payment-correctness bug"* introduced by adding a wallet to one
flow without checking the arms the other flows need).

**Fix:** add `common_enums::PaymentMethodType::Paze` to the `=> Ok(Self::Scheme)` group at
`transformers.rs:1204-1220` (a Paze DPAN is stored as a scheme token at Adyen), and prove the MIT leg
end-to-end. If the MIT leg is genuinely out of scope, remove the `WalletData::Paze` arm from the
`SetupMandate` impl instead of shipping an uncharageable mandate.

#### F12 — [MED] New `SetupMandate` wallet match uses a catch-all where the sibling Authorize match is exhaustive

`transformers.rs:6669-6675`:

```rust
            _ => {
                return Err(IntegrationError::NotImplemented(
                    ("payment method").into(),
                    Default::default(),
                )
                .into())
            }
```

The Authorize wallet match added by the same commits (`transformers.rs:1697-1730`) is fully
exhaustive — it lists `AmazonPayRedirect | RevolutPay | AliPayQr | ApplePayRedirect | …` by name, so
a new `WalletData` variant breaks the build there and forces a decision. The new mandate path uses
`_`, so the next wallet variant added to `WalletData` will silently fall through to an opaque
`NotImplemented` in `SetupRecurring` while the Authorize path refuses to compile. That asymmetry is
the drift PR1779 warned about and the property PR1788's reviewer explicitly endorsed
(*"Inner match is exhaustive (no `_`), so a new variant still breaks compilation here"*).

**Fix:** enumerate the unsupported `WalletData` variants explicitly in the `SetupMandate` match, as
the Authorize match already does.

---

### T4 — Eroded error diagnostics *(PRs 1778, 1779, 1806, 1812, 1873, 1910, 1944, 2003, 2007)*

#### F11 — [MED] Both new error sites use `context: Default::default()` and no `attach_printable`

`transformers.rs:1464-1475`:

```rust
        PazeWalletData::CompleteResponse(complete_response) => serde_json::from_str::<
            PazeDecryptedData,
        >(complete_response.peek())
        .change_context(IntegrationError::InvalidWalletToken {
            wallet_name: "Paze".to_string(),
            context: Default::default(),
        }),
```

and `transformers.rs:1499-1505`:

```rust
        .ok_or_else(|| {
            IntegrationError::MissingRequiredField {
                field_name: "paze_decrypted_data.dynamic_data.dynamic_data_value",
                context: Default::default(),
            }
            .into()
        })
```

Both are **new** error sites (PR1779 distinguished new sites from pre-existing convention:
*"Residual `context: Default::default()` in new error sites … drops diagnostic context"*). Neither
carries an `.attach_printable` (PR1778: *"try to use attach printable with change context so that
while debugging things are clear"*), and the `InvalidWalletToken` message does not say what shape was
expected — the exact failure mode PR2007 raised five separate times against Datatrans
(*"This will mislead integrators debugging Apple Pay failures"*).

**Fix:**

```rust
        .change_context(IntegrationError::InvalidWalletToken {
            wallet_name: "Paze".to_string(),
            context: "Paze completeResponse must deserialize to PazeDecryptedData \
                      (token, dynamicData, billingAddress, consumer, paymentCardNetwork)".into(),
        })
        .attach_printable("failed to deserialize Paze completeResponse")
```

and populate `context` on the `MissingRequiredField` with the `dynamic_data_type` values actually
seen (see F4).

#### F13 — [LOW] `NotImplemented("payment method")` names neither the connector nor the wallet

`transformers.rs:6670-6673` (the `_` arm from F12) returns the bare string `"payment method"`.
The string matches the **PRE-EXISTING** convention of the enclosing dispatcher
(`transformers.rs:6847-6849`), so this is not a regression — but `transformers.rs:1236` in the same
file already uses `utils::get_unimplemented_payment_method_error_message("Adyen")`, and PR1812's
reviewer asked for exactly this upgrade (*"the `NotImplemented` arm should say something equivalent"*).

**Fix:** `IntegrationError::NotImplemented(utils::get_unimplemented_payment_method_error_message("Adyen"), …)`.

**Otherwise clean for T4:** the diff changes no error-response construction — `SetupMandateResponse`
handling reuses the untouched shared `get_adyen_response` / `get_redirection_error_response` family
(`transformers.rs:6874-6895`), so error code / message / reason propagation is unchanged. No T4
regression on the response side.

---

### T5 — Duplicated code that should be a shared helper *(PRs 1812, 1910, 1942, 1944, 1980, 2003)*

#### F8 — [MED] The new wallet `SetupMandate` impl is a ~90-line verbatim copy of the card `SetupMandate` impl

- Card impl: `transformers.rs:6431-6603`
- New wallet impl: `transformers.rs:6605-6787`

Everything from `let amount = get_amount_data_for_setup_mandate(&item);` (card `:6510` / wallet
`:6678`) through the closing `AdyenPaymentRequest { … }` literal is **identical** — the
`shopper_reference` `match` block, `get_recurring_processing_model_for_setup_mandate`, the
`return_url` `ok_or`, `billing_address` via `get_address_info(..).and_then(Result::ok)`,
`get_additional_data_for_setup_mandate`, the `get_adyen_metadata` /
`device_fingerprint` / `platform_chargeback_logic` triple, and all 30 fields of the request literal.
Only three things differ: `payment_method`, `mpi_data`, and the card-only
`testing_data`/`card_holder_name` derivation.

This is the exact pattern PR1812 called out and refactored
(*"the `Card` arm plus the entire `BankRedirect` block was duplicated **verbatim** between
`AirwallexPaymentRequest` and `AirwallexConfirmRequest`, and the card arm a third time in the
SetupMandate builder"*), and PR1980's reviewer's drift argument applies directly
(*"they only stayed identical because I hand-applied the same two fixes three times"*).

**Fix:** extract

```rust
fn build_setup_mandate_request<T: …>(
    item: &AdyenRouterData<RouterDataV2<SetupMandate, …>, T>,
    payment_method: PaymentMethod<T>,
    mpi_data: Option<AdyenMpiData>,
) -> Result<SetupMandateRequest<T>, Error>
```

and have both `TryFrom` impls compute only their `(payment_method, mpi_data)` pair and delegate.

#### F9 — [LOW] `AdyenSamsungPay` construction inlined twice while Paze got helper functions

`transformers.rs:1665-1667`:

```rust
                let samsung_pay_data = AdyenSamsungPay {
                    samsung_pay_token: samsung_pay_data.payment_credential.token_data.data.clone(),
                };
```

and `transformers.rs:6659-6665`:

```rust
                    AdyenPaymentMethod::SamsungPay(Box::new(AdyenSamsungPay {
                        samsung_pay_token: samsung_pay_data
                            .payment_credential
                            .token_data
                            .data
                            .clone(),
                    })),
```

Inconsistent with the Paze half of the same change, which correctly factored
`get_paze_decrypted_data` / `build_paze_network_token_data` / `build_paze_mpi_data` for reuse across
the two flows. Two copies of the same field path, plus two ~7-line comment blocks restating the same
rationale.

**Fix:** `fn build_adyen_samsung_pay(data: &SamsungPayWalletData) -> AdyenSamsungPay`, with the
rationale comment stated once on the helper.

> *Note on the doc comments at `:1657-1663` and `:6653-6654`:* both say the token lives in
> `payment_credential.3_d_s.data` while the code reads `payment_credential.token_data.data`. **Verified
> correct** — `SamsungPayWalletCredentials.token_data` carries `#[serde(rename = "3_d_s")]`
> (`domain_types/src/payment_method_data.rs:963-964`). No finding.

---

### T6 — Capability-surface drift *(PRs 1779, 1811, 1812, 1873, 1944)*

#### F1 — [HIGH] Paze registered as `mandates: NotSupported` by the very PR that implements Paze mandates

`adyen.rs:1394-1404`:

```rust
    // Wallet - Paze (submitted as a network token pass-through)
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::Paze,
        PaymentMethodDetails {
            mandates: FeatureStatus::NotSupported,   // <-- line 1399
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );
```

Commits `9e3c4de32` / `1b1c832d9` add a full `SetupRecurring` (SetupMandate) implementation for
`WalletData::Paze` at `transformers.rs:6639-6651`. The advertised capability says the opposite.
Whichever is right, one of the two is wrong — and PR1812's reviewer was explicit about not shipping
that state (*"I would want to land the proto half first so this one is never in a state where Klarna
is advertised and broken"*). PR1779's severity assessment also turned on advertised support being
accurate (*"Reachability is gated by Razorpay not advertising Skrill support"*).

**Fix:** set `mandates: FeatureStatus::Supported` **and** close F2 (the MIT leg) — or drop the
`WalletData::Paze` arm from the `SetupMandate` impl and leave the registry as-is.

#### F3 — [HIGH] Samsung Pay is never registered in `ADYEN_SUPPORTED_PAYMENT_METHODS`

`adyen.rs` gains **only** the Paze entry (`:1394-1404`); grepping the whole file returns exactly one
new `PaymentMethodType::` addition and no `PaymentMethodType::SamsungPay` anywhere. Yet these commits
add Samsung Pay support to **both** flows — Authorize (`transformers.rs:1651-1670`) and
SetupRecurring (`transformers.rs:6652-6668`) — and a `#[serde(rename = "samsungpay")]`
`AdyenPaymentMethod::SamsungPay` variant (`transformers.rs:228-229`).

Result: Adyen accepts Samsung Pay at the transformer but does not advertise it, so routing and
capability discovery will not select it — the mirror image of F1. This is the PR1944 mechanism
(*"`data/field_probe/glomopay.json` has no `customer_get` key, and `docs-generated/all_connector.md`
consequently reports `Customer.Get = x` (Not Supported) — the very connector this flow was built for"*).

**Fix:** add alongside the Paze block:

```rust
    // Wallet - Samsung Pay (native `samsungpay` payment method; Adyen decrypts server-side)
    adyen_supported_payment_methods.add(
        PaymentMethod::Wallet,
        PaymentMethodType::SamsungPay,
        PaymentMethodDetails {
            mandates: FeatureStatus::Supported,
            refunds: FeatureStatus::Supported,
            supported_capture_methods: adyen_supported_capture_methods.clone(),
            specific_features: None,
        },
    );
```

#### F14 — [MED] `data/field_probe/adyen.json` still records Paze as `not_supported`

The diff touches only the two `.rs` files. `data/field_probe/adyen.json` still contains:

```json
"Paze": {"status": "not_supported",
         "error": "Invalid data format: payment_method. The provided payment method variant is empty or not supported by this flow"}
```

under `flows.authorize`, and the file has no `setup_recurring` per-method breakdown for either wallet.
Until the probe is regenerated, `docs-generated/*` and the SDK examples will keep telling integrators
Paze is unsupported on Adyen. PR1811, PR1812, PR1873 and PR1944 all treated exactly this artifact as
review-relevant.

**Fix:** regenerate `data/field_probe/adyen.json` (live sandbox run, per PR1812's
*"I have not hand-edited … the auto-fix job will regenerate it on this push"*) and let
`docs-generated` follow.

---

### T7 — Secrets / PII typing *(PRs 1788, 1820, 1944, 1986, 2004)* — **CLEAN**

No violation. All new token/PII fields stay inside `Secret<T>`:
`AdyenSamsungPay.samsung_pay_token: Secret<String>` (`transformers.rs:1096-1099`), and every field
`build_paze_network_token_data` reads (`token.payment_token: cards::NetworkToken`,
`token_expiration_month/_year: Secret<String>`, `billing_address.name`, `consumer.full_name`) is
already a masked type and is moved without unwrapping. The single `.peek()` at `transformers.rs:1469`
is on `PazeWalletData::CompleteResponse` and is consumed immediately by `serde_json::from_str` — the
result is a `PazeDecryptedData` whose sensitive fields are `Secret`, so nothing is stored, logged, or
re-wrapped in the clear. The pattern matches the pre-existing Cybersource Paze handler
(`cybersource/transformers.rs:2487`). No new `Debug`/`Display` impls, no new logging.

---

### T8 — Panics / unvalidated untrusted data *(PRs 1811, 1812, 1953, 2003)*

**No panic risk introduced.** Verified by grep over the added lines: the diff contains no `.unwrap()`,
no `.expect()`, no `panic!`, no slice/array indexing, and no arithmetic. Clean on the panic half of
this theme.

#### F7 — [MED] Paze expiry month/year forwarded to Adyen with no validation

`transformers.rs:1517-1522`:

```rust
    AdyenNetworkTokenData {
        number: paze_decrypted_data.token.payment_token.clone(),
        expiry_month: paze_decrypted_data.token.token_expiration_month.clone(),
        expiry_year: domain_utils::expand_expiry_year_to_four_digits(
            &paze_decrypted_data.token.token_expiration_year,
        ),
```

For `PazeWalletData::CompleteResponse`, `paze_decrypted_data` is produced by a raw
`serde_json::from_str` (`transformers.rs:1467-1469`) over a merchant/SDK-supplied string —
`token_expiration_month` and `token_expiration_year` are plain `Secret<String>`
(`domain_types/src/router_data.rs:3790-3791`) with no format constraint. The month is forwarded
verbatim. The year goes through
`expand_expiry_year_to_four_digits` (`domain_types/src/utils.rs:654-662`):

```rust
pub fn expand_expiry_year_to_four_digits(year: &Secret<String>) -> Secret<String> {
    let y = year.peek();
    if y.len() == 2 {
        let century = common_utils::date_time::now().year() / 100;
        Secret::new(format!("{century}{y}"))
    } else {
        Secret::new(y.clone())
    }
}
```

— no digit check whatsoever. This is verbatim the gap JeevaRamu0104 blocked on in **PR2003**:

> *"The year half isn't validated at all. `expand_expiry_year_to_four_digits` just prefixes the
> century for any 2-char string with no digit check, so `\"12ab\"` comes back as …
> `card_exp_year = \"20ab\"` and we hand that to Hyperswitch as the refreshed card. The month goes
> through `CardExpirationMonth::try_from`; the year goes through nothing."*

Here **neither** goes through anything. `"ab"` becomes `"20ab"` on the wire; `"1"` / `"013"` /
`""` pass through as-is. Note that `expand_expiry_year_to_four_digits` also uses `y.len()` (bytes),
so the byte-length-vs-char-boundary confusion PR2003 flagged is latent in the same helper.

**Fix:** validate before building the token, mirroring the guard PR2003 landed
(`eded7a33c`):

```rust
    let month = cards::validate::CardExpirationMonth::try_from(
        paze_decrypted_data.token.token_expiration_month.clone(),
    ).change_context(IntegrationError::InvalidDataFormat { field_name: "paze token_expiration_month" })?;
    let raw_year = paze_decrypted_data.token.token_expiration_year.peek();
    if !matches!(raw_year.len(), 2 | 4) || !raw_year.bytes().all(|b| b.is_ascii_digit()) {
        return Err(… InvalidDataFormat("paze token_expiration_year") …);
    }
```

(`build_paze_network_token_data` would need to return `Result<_, Error>`; both call sites at
`transformers.rs:1693` and `:6642` are already in `?`-capable contexts.)

---

### T9 — Amount handling bypassing the framework converter *(PRs 1788, 1789)* — **CLEAN**

The new wallet `SetupMandate` impl uses the pre-existing shared
`get_amount_data_for_setup_mandate(&item)` (`transformers.rs:6678`, helper at `:6921+`), identical to
the card path at `:6510`. No hand-rolled amount formatting, no `get_amount_as_i64().to_string()`, no
minor-unit bypass anywhere in the diff.

---

### T10 — Unnecessary clones *(PRs 1779, 1788, 1812)*

#### F10 — [MED] `PazeDecryptedData` is deep-cloned out of its `Box`, and derived twice per Authorize

Two related problems.

**(a)** `transformers.rs:1466`:

```rust
        PazeWalletData::Decrypted(paze_decrypted_data) => Ok(*paze_decrypted_data.clone()),
```

`PazeWalletData::Decrypted` holds a `Box<PazeDecryptedData>`
(`domain_types/src/payment_method_data.rs:987`) — boxed specifically so the large variant is not
copied around. This line clones the whole struct (client_id, token, `Vec<PazeDynamicData>`,
billing address, consumer, eci) and immediately deref-moves it out of the box. Every consumer
(`build_paze_network_token_data`, `build_paze_mpi_data`) takes `&PazeDecryptedData`, so nothing needs
ownership. This is PR1812's point verbatim: *"Borrowing `payment_method_data` instead of matching on
`.clone()` also drops two clones per request."*

**(b)** In the **Authorize** flow, `get_paze_decrypted_data` is called **twice for the same request** —
once at `transformers.rs:1692` (building the `networkToken` payment method) and once at
`transformers.rs:2661` (building `mpiData`):

```rust
// :1692
                let paze_decrypted_data = get_paze_decrypted_data(paze_wallet_data)?;
                Ok(Self::NetworkToken(Box::new(build_paze_network_token_data(…))))
// :2661
                let paze_decrypted_data = get_paze_decrypted_data(paze_wallet_data)?;
                Some(build_paze_mpi_data(&paze_decrypted_data)?)
```

So a `CompleteResponse` Paze payload is JSON-parsed twice per authorization, and a `Decrypted`
payload is deep-cloned twice. The new `SetupMandate` impl gets this right (`:6640`, one call feeding
both), which makes the Authorize path the odd one out.

**Fix:** change `get_paze_decrypted_data` to return `Cow<'_, PazeDecryptedData>` (borrow the
`Decrypted` arm, own the parsed `CompleteResponse` arm), and hoist the single call in the Authorize
builder so `payment_method` and `mpi_data` share one derivation, as the mandate builder does.

---

### T11 — Dead / unreachable code *(PRs 1811, 1812, 1855, 1910, 1944, 1952, 2003, 2013)* — **CLEAN**

No dead code introduced. `AdyenPaymentMethod::SamsungPay` is constructed at `:1669` and `:6659`;
`AdyenSamsungPay` is constructed at both; all four new helper fns
(`get_paze_decrypted_data`, `get_paze_token_cryptogram`, `build_paze_network_token_data`,
`build_paze_mpi_data`) have live callers in both flows; `PAZE_DEFAULT_ECI` is read at `:1551`.
No commented-out blocks, no orphaned `pub` items, no inline `mod tests` (PR2013's request respected).

*Caveat:* if F2 is left unfixed, the `WalletData::Paze` arm of the `SetupMandate` impl
(`transformers.rs:6639-6651`) produces mandates that no flow can use — functionally the
"unreachable value" situation PR1952 asked to be made explicit rather than left implicit.

---

### T12 — Missing serialization coverage *(PRs 1812, 1944, 1994, 2006)* — **NOT MET**

363 added lines introduce four new wire-shape decisions with **no automated coverage**:

1. `AdyenSamsungPay` serializes as `{"type":"samsungpay","samsungPayToken":"…"}` — the
   `#[serde(rename = "samsungpay")]` tag at `transformers.rs:228` and the
   `#[serde(rename = "samsungPayToken")]` field at `:1097` are untested. Note the enclosing enum has
   three variants sharing `#[serde(rename = "scheme")]` (`:215-232`), so tag correctness here is not
   self-evident.
2. Paze → `{"type":"networkToken", …}` plus a sibling `mpiData` block.
3. `mpiData` field names/values for the Paze case (`tokenAuthenticationVerificationValue`, `eci`).
4. The `CompleteResponse` → `PazeDecryptedData` deserialization contract.

PR1812's reviewer's phrasing applies directly: *"pure serialization logic that unit-tests cheaply and
will otherwise drift silently."* Per PR2013 the home for this is a separate
`connectors/adyen/test.rs` (Adyen already has one), **not** an inline `mod tests`.

**Fix:** add wire-shape assertions in `connectors/adyen/test.rs` — serialize a Paze
`AdyenPaymentRequest` and a Samsung Pay one and assert the JSON object keys, exactly as PR1812 did
(*"Added a test that asserts the wire shape rather than the struct, so this cannot silently regress"*).

---

## Themes with no violation in the new code

| Theme | Verdict |
|---|---|
| **T7 — Secrets / PII** | Clean. All tokens and PII remain in `Secret<T>`; the single `.peek()` feeds `serde_json::from_str` and is not stored or logged. |
| **T8 — Panics** (panic half) | Clean. No `.unwrap()`, `.expect()`, `panic!`, indexing or arithmetic anywhere in the diff. (Validation half → F7.) |
| **T9 — Amount handling** | Clean. Reuses the shared `get_amount_data_for_setup_mandate`; no minor-unit bypass. |
| **T11 — Dead code** | Clean. Every new item has a live caller; no inline `mod tests`. |
| **T4 — Error-response propagation** | Clean on the response side; `SetupMandateResponse` handling reuses the untouched shared response mappers. No status-mapping change of any kind in this diff. |

## Pre-existing patterns moved or copied, not newly introduced

- `get_address_info(..).and_then(Result::ok)` at `transformers.rs:6698`, `:6741` — copied verbatim
  from the card `SetupMandate` impl (`:6491`, `:6555`) and the Authorize wallet path (`:2692`).
  **PRE-EXISTING**; counted only under F8 (duplication).
- `directory_response` / `authentication_response` hardcoded to `Success` — identical lines already
  at `:2620-2621`, `:2642-2643`, `:3476-3477`. **PRE-EXISTING pattern**; see F6.
- `metadata: None` and `get_adyen_metadata(..).expose_option()` in the new request literal — verbatim
  from the card `SetupMandate` impl. **PRE-EXISTING**.
- `NotImplemented("payment method")` string — the dispatcher convention at `:6847`. **PRE-EXISTING**;
  see F13.
- `PaymentType::try_from`'s `_ =>` arm (`:1235`) — the function is **PRE-EXISTING and untouched**;
  only its *reachability for Paze* is new. See F2.
- `expand_expiry_year_to_four_digits` (`domain_types/src/utils.rs:654`) — **PRE-EXISTING shared
  helper**, unmodified; the finding (F7) is that the new code feeds it unvalidated remote data, which
  is the same misuse PR2003 blocked on.

---

## Recommended fix order

1. **F2** — add `PaymentMethodType::Paze` to `PaymentType::try_from`'s `Scheme` group, or drop the
   Paze `SetupMandate` arm. (Blocking: mandates are otherwise uncharageable.)
2. **F1 + F3** — reconcile `ADYEN_SUPPORTED_PAYMENT_METHODS` with what the code now does: fix Paze
   `mandates`, register Samsung Pay.
3. **F4** — remove the arbitrary-dynamic-data cryptogram fallback.
4. **F5 / F7** — stop defaulting `eci`, validate the Paze expiry.
5. **F10 / F8 / F9** — one Paze derivation per request; extract the shared `SetupMandate` builder and
   a Samsung Pay helper.
6. **F11 / F12 / F13** — diagnostics and exhaustive matching.
7. **F14 + T12** — regenerate `data/field_probe/adyen.json`; add wire-shape tests in
   `connectors/adyen/test.rs`.
