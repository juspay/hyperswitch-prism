# Adyen Paze + Samsung Pay — UCS Shadow Validation Report

**Repo under validation:** `/home/infamous/hyperswitch-prism5` @ `feat/adyen_grace`
**Commits:** `8a6d97cc5` (Authorize Paze), `9aac731f7` (Authorize SamsungPay), `9e3c4de32` (SetupRecurring Paze), `1b1c832d9` (SetupRecurring SamsungPay)
**Reference:** `/home/infamous/hyperswitch5` @ `63cfcc3e5e` — `crates/hyperswitch_connectors/src/connectors/adyen{,/transformers}.rs`
**Validation service:** https://github.com/juspay/ucs-shadow-validation-service (shallow clone, HEAD as of 2026-08-03)
**Date:** 2026-08-03

---

## 1. Verdict

**Did the shadow validation service run end-to-end against live traffic? NO.**

**Did any part of the service run for real? YES — partially.** The `validation-service-headless`
receiver, its Redis store, and its actual comparison engine (`report/comparator/jsonComparator.js`,
`controller/router-data/index.js`) were installed and executed locally. What could **not** be run is
the *traffic-producing* half: a hyperswitch router and a MITM proxy generating genuine paired
hyperswitch/UCS router data from real Paze and Samsung Pay payments.

Everything reported in §4 is therefore **static parity analysis**, run through the service's
**real comparator** on payloads derived by reading both implementations — **not** captured
production traffic. This distinction is maintained throughout.

---

## 2. What the service actually does

Read from the cloned repo, this is a two-part comparison harness, not a test runner:

| Component | Role |
|---|---|
| `mitm-proxy/` (mitmproxy + `forward_code.py`) | Intercepts outbound HTTPS from **both** the hyperswitch router and the UCS `grpc-server`, forwards each connector request/response to the validation service (`MP_02`/`MP_03` = router-data / connector-service request intercepted) |
| `validation-service-headless/` | Express receiver. `POST /api/receive` (proxy-intercepted connector traffic) and `POST /api/router-data` (router-posted RouterData pairs). Runs `compareJson` and stores results in Redis |
| `validation-service-web/` | React diff viewer over the Redis-stored results |

Two independent comparisons happen:

1. **Connector-request parity** — the Adyen HTTP request body each side builds, captured by the proxy.
2. **Router-data parity** — the full serialized `RouterData<F, Req, Resp>` from each side. This is
   posted *directly by the hyperswitch router*, not by the proxy. Confirmed at
   `/home/infamous/hyperswitch5/crates/hyperswitch_interfaces/src/helpers.rs:47-100`
   (`serialize_comparison_results_and_send`) → `helpers.rs:143-195` (`send_comparison_data`,
   which sets `x-flow: router-data`, `x-connector`, `x-sub-flow`, `x-request-id`).

The router serializes **both** result quadrants — `(Ok,Ok)`, `(Ok,Err)`, `(Err,Ok)`, `(Err,Err)` —
so error-path divergence is visible as a diff (`helpers.rs:65-75`). The comparator emits
`keyDiff` (field present on one side only), `valueDiff`, `typeDiff`, with PCI/PII values masked
to `****`.

### Required to run end-to-end

- Docker + docker-compose (README's only supported path)
- A **hyperswitch router** built from `hyperswitch5`, with `[proxy]` + `mitm_ca_certificate` and a
  `comparison_service` config, plus DB/Redis and a merchant with a live Adyen MCA
- A **UCS `grpc-server`** built from the branch under test, with `[proxy] mitm_proxy_enabled = true`
  and `mitm_ca_cert`
- UCS enabled + `ucs_rollout_config_..._shadow` set per merchant/connector/PM/flow via `/configs/`
- Genuine Paze and Samsung Pay SDK payloads

---

## 3. Runnability in this environment — precise blockers

| Requirement | Status | Evidence |
|---|---|---|
| Clone the service | **OK** | shallow clone succeeded |
| Node 22 | **OK** | `v22.14.0` |
| Redis | **OK** | `redis-cli -p 6379 ping` → `PONG` |
| Postgres | **OK** | listening `127.0.0.1:5432` |
| `validation-service-headless` runs | **OK** | started on `:9711`; `/api/router-data` returned 200 and stored to Redis (see §5) |
| `validation-service-web` diff UI | **BLOCKED** | build missing: `ENOENT .../validation-service-web/dist/index.html`. Non-fatal (headless API works) |
| **Docker / docker-compose stack** | **BLOCKED** | `docker info` → `permission denied ... /var/run/docker.sock`. Socket is `srw-rw---- root:docker`; current user `uid=1008(infamous) groups=1008(infamous)` — not in `docker` group |
| **Hyperswitch router binary** | **BLOCKED (hard)** | `/home/infamous/hyperswitch5/target` **does not exist** — the router has never been built. A debug build of `crates/router` needs well over the **12 GB free** on a root disk already at **100%** (`/dev/nvme0n1p2 1.9T 1.8T 12G 100%`). Per instructions, no heavy cargo build was attempted |
| Comparison-service config in hyperswitch5 | **MISSING** | no `comparison_service` / `mitm_ca_certificate` key in `config/development.toml`; `ucs_only_connectors` (line 1571) does **not** include `adyen` |
| MITM proxy | **BLOCKED** | requires Docker. Another user's stack is bound to `:18081-18083` — not ours, not usable |
| Merchant + Adyen MCA + shadow rollout config | **NOT PRESENT** | no router to configure |
| Genuine Samsung Pay SDK token | **NOT AVAILABLE** | per prior live testing, Adyen rejects with `11_002` / `11_006` (token decryption failure) |

### Primary blocker (one line)

> **The hyperswitch router — the only component that produces router-data pairs — is not built and cannot be built: `hyperswitch5/target` is absent and the root disk is at 100% with ~12 GB free. Compounding this, the Docker socket is not accessible to this user, so the documented `docker-compose` path is unavailable.**

### What live traffic could and could not have validated anyway

Prior grpcurl testing established:

- **Paze Authorize → CHARGED**, **Paze SetupRecurring → CHARGED** against the Adyen sandbox.
  This proves the UCS request is *accepted by Adyen*. It does **not** prove parity — the hyperswitch
  side was never run for the same input, and Adyen accepts both a 2-digit and a 4-digit `expiryYear`,
  and accepts `mpiData.tokenAuthenticationVerificationValue` without validating its provenance.
  **Every mismatch in §4.1 is invisible to a "did it charge?" test.**
- **Samsung Pay → Adyen `11_002` / `11_006`** (token decryption failure), because no genuine Samsung
  device SDK token exists. Both implementations forward the *same* opaque bytes
  (`payment_credential.token_data.data`), so this failure is **symmetric** and is not evidence of a
  UCS defect — but it also means the Samsung Pay path is **unvalidated end-to-end on either side**.

---

## 4. Static router-data / request parity analysis

Method: identical fixture input on both sides; derive the exact serialized payload each
implementation produces; run the service's real `compareJson` over the pair. Both
`AdyenPaymentRequest` and `AdyenMpiData` use `#[serde_with::skip_serializing_none]` on both
sides, so `None` → key omitted (modelled accordingly).

### 4.1 NEW mismatches introduced by these four commits

---

#### M1 — `mpiData.tokenAuthenticationVerificationValue` carries a completely different value — **HIGH**

- UCS: `crates/integrations/connector-integration/src/connectors/adyen/transformers.rs:1544`
  → `get_paze_token_cryptogram()` at `transformers.rs:1479-1503` — picks the
  `dynamic_data` entry whose `dynamic_data_type == "CRYPTOGRAM_3DS"` (falling back to the first
  entry with any value), i.e. the **TAVV cryptogram**.
- HS5: `crates/hyperswitch_connectors/src/connectors/adyen/transformers.rs:4084`
  → `token_authentication_verification_value: Some(paze_data.token.payment_account_reference)`,
  i.e. the **PAR**.

These are different fields of the Paze payload with different semantics. Adyen's sandbox accepted
the UCS value, so the live CHARGED result did not surface this.

Note: UCS is very likely the *correct* one (a TAVV belongs in a TAVV field; a PAR does not) and HS5
looks like a latent bug. **But shadow validation compares, it does not judge** — this will fire as a
`valueDiff` on every single Paze transaction and must be reconciled deliberately, not left to
diverge silently.

---

#### M2 — `paymentMethod.expiryYear` differs in width (2-digit vs 4-digit) — **MEDIUM**

- UCS: `transformers.rs:1520-1522` — `domain_utils::expand_expiry_year_to_four_digits(&token.token_expiration_year)`
  (`crates/types-traits/domain_types/src/utils.rs:654-662`: 2-char input → prefixed with current century).
- HS5: `adyen/transformers.rs:2706` — `expiry_year: paze_decrypted_data.token.token_expiration_year` (verbatim).

Paze emits a 2-digit year, so UCS sends `"2027"` where hyperswitch sends `"27"`. Adyen tolerates
both, so this is silent in live traffic and loud in a shadow diff. Affects **Authorize and SetupRecurring**.

---

#### M3 — `paymentMethod.holderName` has an extra fallback in UCS — **MEDIUM**

- UCS: `transformers.rs:1523-1528` — `billing_address.name` → `get_optional_billing_full_name()` → **`consumer.full_name`**.
- HS5: `adyen/transformers.rs:2708-2711` — `billing_address.name` → `get_optional_billing_full_name()`. No third fallback.

When neither Paze billing name nor router billing name is present, hyperswitch **omits** `holderName`
and UCS **sends** `consumer.full_name` → a `keyDiff` (present on one side only), not merely a value diff.

Secondary, same field: `AdyenNetworkTokenData.holder_name` (`transformers.rs:184-193`) carries **no**
`#[serde(skip_serializing_if = "Option::is_none")]`, unlike its sibling `brand` and
`network_payment_reference`. The struct as a whole is not `skip_serializing_none`. HS5's
`AdyenPazeData` (`adyen/transformers.rs:1440-1449`) **is** `#[serde_with::skip_serializing_none]`.
So on any other caller of `AdyenNetworkTokenData` where `holder_name` is `None`, UCS emits
`"holderName": null` where hyperswitch omits the key.

---

#### M4 — `mpiData.eci` is defaulted in UCS, omitted in hyperswitch — **MEDIUM**

- UCS: `transformers.rs:1548-1552` — `paze_decrypted_data.eci.unwrap_or_else(|| PAZE_DEFAULT_ECI)`
  where `PAZE_DEFAULT_ECI = "05"` (`transformers.rs:124`).
- HS5: `adyen/transformers.rs:4086` — `eci: paze_data.eci.clone()` — `Option`, omitted when `None`.

`PazeDecryptedData.eci` is `Option<String>` on **both** sides
(`domain_types/src/router_data.rs:3783`; `hyperswitch_domain_models/src/router_data.rs:651`), and is
frequently absent. Result: UCS sends `"eci": "05"`, hyperswitch sends no `eci` at all — a `keyDiff`.
The `"05"` default is defensible on its own merits, but it is a unilateral divergence.

---

#### M5 — Samsung Pay is absent from `ADYEN_SUPPORTED_PAYMENT_METHODS` — **MEDIUM**

- UCS: `crates/integrations/connector-integration/src/connectors/adyen.rs:1394-1404` adds **only**
  `PaymentMethodType::Paze`. Grep for `PaymentMethodType::SamsungPay` in that file returns **nothing**.
- HS5: `crates/hyperswitch_connectors/src/connectors/adyen.rs:2985-2994` registers `SamsungPay`
  **and** `2996-3005` registers `Paze`.

Samsung Pay Authorize and SetupRecurring were implemented (commits `9aac731f7`, `1b1c832d9`) but the
connector never advertises the capability. Capability metadata feeds routing/eligibility and the
generated docs, so this is a real functional gap, not cosmetic.

---

#### M6 — Paze declares `mandates: NotSupported` while SetupRecurring/Paze is implemented — **LOW**

`adyen.rs:1394-1404` sets `mandates: FeatureStatus::NotSupported` for Paze. Commit `9e3c4de32`
implements SetupRecurring for Paze. This **matches** hyperswitch5 (`adyen.rs:3000`, also
`NotSupported`), so it is *not* a parity mismatch — but it is internally inconsistent with the branch's
own new capability, and if UCS is meant to be the source of truth the flag should move.

---

#### M7 — divergent error for undecrypted Paze (`CompleteResponse`) — **MEDIUM** (error-quadrant router-data diff)

The router chooses which Paze representation to send over gRPC at
`/home/infamous/hyperswitch5/crates/router/src/core/unified_connector_service.rs:113-125`:
`PaymentMethodToken::PazeDecrypt` present → `PazeData::DecryptedData`; otherwise → fall through to
`transformers.rs:4006-4018`, which sends `PazeData::CompleteResponse(complete_response)`.

`complete_response` is a **JWE compact serialization**, not JSON — proven by the router's own
decryptor at `crates/router/src/core/payments/helpers.rs:7772-7790` (`.split('.')`, base64url-decode
element 1, `jwe::deserialize_compact`).

For that no-token case the two sides fail differently:

- UCS: `transformers.rs:1467-1476` — `serde_json::from_str::<PazeDecryptedData>(complete_response.peek())`
  on a JWE → always `Err` → `IntegrationError::InvalidWalletToken { wallet_name: "Paze" }`.
- HS5: `adyen/transformers.rs:2718-2722` — `ConnectorError::NotImplemented(...)`
  (with the literal string **`"Cybersource"`** — an unrelated pre-existing HS5 bug).

Both fail, so no payment-outcome regression — but the router serializes both error quadrants, so
this surfaces as a `valueDiff` on `response.Err.code` / `.message` / `.reason` / `.status_code`.
Reproduced live against the running service in §5.

Additionally, UCS's `CompleteResponse` arm is **unreachable-as-intended**: it can only ever succeed
if some caller hands it a JSON-serialized `PazeDecryptedData` as a string (which is what hand-built
grpcurl payloads do). It cannot succeed on real router traffic.

---

#### M8 — `customer_id` is mandatory in UCS SetupRecurring, optional in hyperswitch — **LOW**

- UCS: `transformers.rs:6977-6985` (`get_recurring_processing_model_for_setup_mandate`) — hard error
  `MissingRequiredField { field_name: "customer_id" }` when absent.
- HS5: `adyen/transformers.rs:2095` (`get_recurring_processing_model`) — `item.get_connector_customer_id().ok()`, optional.

A SetupRecurring without `customer_id` fails in UCS and proceeds in hyperswitch → `(Err, Ok)` quadrant diff.

---

### 4.2 Pre-existing UCS-vs-hyperswitch SetupMandate divergences INHERITED by the new wallet path

These are **not introduced** by the four commits — they already exist in UCS's card SetupMandate impl
at base commit `45351c251` (verified: `get_amount_data_for_setup_mandate` and `application_info: None`
both present in `git show 45351c251:...transformers.rs`). The new wallet impl
(`transformers.rs:6612-6790`) copies the same field set, so it inherits them. They **will** appear in
any real shadow run for SetupRecurring and would otherwise be misread as regressions.

Root cause of the whole class: **hyperswitch does not have a separate SetupMandate request builder for
Adyen.** `crates/hyperswitch_connectors/src/connectors/adyen.rs:521-538` converts SetupMandate router
data into Authorize router data (`convert_setup_mandate_router_data_to_authorize_router_data`,
`crates/hyperswitch_connectors/src/utils.rs:7718-7778`) and reuses the Authorize builder verbatim.
UCS instead maintains a **hand-written duplicate** — so every field the duplicate sets differently
is a permanent diff.

| # | Field | hyperswitch | UCS | Note |
|---|---|---|---|---|
| P1 | `amount.value` | hard `0` (`utils.rs:7732-7734`, `minor_amount: MinorUnit::new(0)`) | `request.amount.unwrap_or(0)` (`transformers.rs:6938`) | **Most material.** UCS can send a **non-zero** amount for a zero-auth mandate setup |
| P2 | `shopperName` | `None` on the wallet path (`adyen/transformers.rs:4161`) | `get_shopper_name(billing)` (`transformers.rs:6744-6749`) | `keyDiff` |
| P3 | `countryCode` | `None` on the wallet path (`adyen/transformers.rs:4166`) | `get_country_code(billing)` (`transformers.rs:6763-6765`) | `keyDiff` |
| P4 | `applicationInfo` | `get_application_info(item)`; `partner_merchant_identifier_details` **is** preserved by the conversion (`utils.rs:7772-7775`) | hard `None` (`transformers.rs:6784`) | `keyDiff` — UCS **drops partner/platform attribution** on SetupRecurring |
| P5 | `merchantOrderReference` | `None` (conversion sets `merchant_order_reference_id: None`, `utils.rs:7756`) | `request.merchant_order_id` (`transformers.rs:6777`) | `keyDiff` |
| P6 | `shopperLocale` | `None` (conversion sets `locale: None`, `utils.rs:7764`) | `request.locale` (`transformers.rs:6768`) | `keyDiff` when locale set |
| P7 | `metadata` | `None` (conversion sets `metadata: None`, `utils.rs:7750`) | `None` (`transformers.rs:6781`) | **agrees** |
| P8 | `channel` | `get_channel_type(pm_type)`; conversion sets `payment_method_type: None` and `get_channel_type` (`adyen/transformers.rs:2244-2251`) only maps GoPay/Vipps → `None` | `None` | **agrees** |

### 4.3 Verified as matching (no action)

- **Samsung Pay `paymentMethod` is byte-identical on both sides and in both flows.**
  UCS `AdyenSamsungPay { samsung_pay_token }` (`transformers.rs:1096-1099`) vs HS5
  `SamsungPayPmData { samsung_pay_token }` (`adyen/transformers.rs:980-983`); both source
  `payment_credential.token_data.data` (UCS `transformers.rs:1666` and `6660-6664`;
  HS5 `adyen/transformers.rs:2695-2699`). Serde tag resolves to `"samsungpay"` on both
  (UCS explicit `#[serde(rename = "samsungpay")]`; HS5 via enum-level `rename_all = "lowercase"`).
- **Paze `paymentMethod.type` = `"networkToken"` on both** (UCS `transformers.rs:219`; HS5 `adyen/transformers.rs:809-810`).
- `paymentMethod.number` / `expiryMonth` / `brand` (`get_adyen_card_network`) / `networkPaymentReference` — identical.
- `mpiData.directoryResponse` / `authenticationResponse` = `Success`, `cavv` = `None` — identical.
- `cvc`: HS5 declares it and skips when `None`; UCS omits the field entirely → identical wire output.
- **Response-side extraction is shared and identical** — mandate id, network txn id, status:
  UCS `get_adyen_response` (`transformers.rs:4883-4917`) and HS5 `get_adyen_response`
  (`adyen/transformers.rs:4586-4596`) both derive `mandate_reference.connector_mandate_id` from
  `additionalData.recurringDetailReference` with `payment_method_id: None`, and `network_txn_id`
  the same way. UCS's SetupMandate response transformer (`transformers.rs:6856-6920`) routes through
  the same helpers as Authorize. **No response-side mismatch found.**
- `shopperStatement`, `shopperEmail` (non-Paypal), `shopperIP`, `billingAddress`, `deliveryAddress`,
  `telephoneNumber` — same sources.

---

## 5. Actual tool output

### 5.1 Real `compareJson` run — Adyen connector request bodies

Ran the service's own comparator (`validation-service-headless/report/comparator/jsonComparator.js`,
prod deps installed, `IGNORE_KEYS` / `ROUTER_DATA_IGNORE_KEYS` from `.env.example`) over payloads
derived per §4. Values under PCI/PII patterns are masked to `****` by the comparator itself.
Scoped to `paymentMethod` + `mpiData` + the SetupMandate-level fields; other fields come from
shared code paths untouched by these commits.

```
==============================================================================
Adyen / Authorize / Paze  -> POST /v68/payments
------------------------------------------------------------------------------
keyDiff=2  valueDiff=2  typeDiff=0  -> DIFFERENCES FOUND
{
  "keyDiff": {
    "paymentMethod.holderName": { "hyperswitch": false, "ucs": true },
    "mpiData.eci":              { "hyperswitch": false, "ucs": true }
  },
  "valueDiff": {
    "paymentMethod.expiryYear":                   { "hyperswitch": "****", "ucs": "****" },
    "mpiData.tokenAuthenticationVerificationValue":{ "hyperswitch": "****", "ucs": "****" }
  },
  "typeDiff": {}
}

==============================================================================
Adyen / Authorize / SamsungPay  -> POST /v68/payments
------------------------------------------------------------------------------
keyDiff=0  valueDiff=0  typeDiff=0  -> NO DIFFERENCES
{ "keyDiff": {}, "valueDiff": {}, "typeDiff": {} }

==============================================================================
Adyen / SetupRecurring (SetupMandate) / Paze  -> POST /v68/payments
------------------------------------------------------------------------------
keyDiff=6  valueDiff=3  typeDiff=0  -> DIFFERENCES FOUND
{
  "keyDiff": {
    "paymentMethod.holderName": { "hyperswitch": false, "ucs": true },
    "mpiData.eci":              { "hyperswitch": false, "ucs": true },
    "applicationInfo":          { "hyperswitch": true,  "ucs": false },
    "shopperName":              { "hyperswitch": false, "ucs": true },
    "countryCode":              { "hyperswitch": false, "ucs": true },
    "merchantOrderReference":   { "hyperswitch": false, "ucs": true }
  },
  "valueDiff": {
    "amount.value":                                { "hyperswitch": 0, "ucs": 1000 },
    "paymentMethod.expiryYear":                    { "hyperswitch": "****", "ucs": "****" },
    "mpiData.tokenAuthenticationVerificationValue": { "hyperswitch": "****", "ucs": "****" }
  },
  "typeDiff": {}
}

==============================================================================
Adyen / SetupRecurring (SetupMandate) / SamsungPay  -> POST /v68/payments
------------------------------------------------------------------------------
keyDiff=4  valueDiff=1  typeDiff=0  -> DIFFERENCES FOUND
{
  "keyDiff": {
    "applicationInfo":        { "hyperswitch": true,  "ucs": false },
    "shopperName":            { "hyperswitch": false, "ucs": true },
    "countryCode":            { "hyperswitch": false, "ucs": true },
    "merchantOrderReference": { "hyperswitch": false, "ucs": true }
  },
  "valueDiff": { "amount.value": { "hyperswitch": 0, "ucs": 1000 } },
  "typeDiff": {}
}
```

### 5.2 Real `POST /api/router-data` run — M7 error-quadrant divergence

Service started on `:9711` against local Redis; a router-data pair modelling M7 posted with
`x-flow: router-data`, `x-sub-flow: SetupMandate`, `x-connector: adyen`. Verbatim service log:

```
[ROUTER DATA - PAYMENT ID OK] Payment IDs validated successfully
  x-request-id=req_paze_setup_001  payment_id=pay_adyen_paze_setup_001
[ROUTER DATA - STATUS START] code=VS_52  hyperswitch_status=Failure  ucs_status=Failure
[ROUTER DATA - DIFF] Router data comparison completed - differences found   code=VS_48
  comparison.summary = { differences_found: true, total_key_differences: 0,
                         total_value_differences: 4, total_type_differences: 0 }
  differences.valueDiff = {
    "response.Err.code":        { "hyperswitch": "IR_00",
                                  "ucs": "INVALID_WALLET_TOKEN" },
    "response.Err.message":     { "hyperswitch": "Payment method type not supported",
                                  "ucs": "Invalid wallet token received for Paze" },
    "response.Err.reason":      { "hyperswitch": "Cybersource is not implemented",
                                  "ucs": "failed to parse PazeDecryptedData from complete_response" },
    "response.Err.status_code": { "hyperswitch": 501, "ucs": 400 }
  }
[ROUTER DATA - STATUS END] code=VS_53  comparison_status=differences_found
[ROUTER DATA - STORED] code=VS_45
  redisKey=router_data_adyen_SetupMandate_req_paze_setup_001_pay_adyen_paze_setup_001
```

The receiver, comparator, payment-id validation, and Redis persistence all executed for real.
**The input pair was constructed from static analysis, not captured from a live payment.**

---

## 6. Summary of mismatches

| ID | Field / concern | file:line (UCS) | Severity | New? |
|----|---|---|---|---|
| M1 | `mpiData.tokenAuthenticationVerificationValue` — TAVV vs PAR | `adyen/transformers.rs:1544`, `:1479` | **HIGH** | yes |
| M2 | `paymentMethod.expiryYear` — 4-digit vs 2-digit | `adyen/transformers.rs:1520` | MEDIUM | yes |
| M3 | `paymentMethod.holderName` — extra `consumer.full_name` fallback; no `skip_serializing_if` | `adyen/transformers.rs:1528`, `:188` | MEDIUM | yes |
| M4 | `mpiData.eci` — defaulted `"05"` vs omitted | `adyen/transformers.rs:1551`, `:124` | MEDIUM | yes |
| M5 | Samsung Pay missing from `ADYEN_SUPPORTED_PAYMENT_METHODS` | `adyen.rs:1394-1404` | MEDIUM | yes |
| M6 | Paze `mandates: NotSupported` vs implemented SetupRecurring | `adyen.rs:1398` | LOW | yes |
| M7 | Undecrypted-Paze error divergence; `CompleteResponse` arm cannot parse a JWE | `adyen/transformers.rs:1467-1476` | MEDIUM | yes |
| M8 | `customer_id` mandatory in UCS SetupRecurring | `adyen/transformers.rs:6977-6985` | LOW | yes |
| P1 | SetupMandate `amount.value` — `request.amount` vs hard `0` | `adyen/transformers.rs:6938` | MEDIUM | pre-existing |
| P2 | SetupMandate `shopperName` sent by UCS only | `adyen/transformers.rs:6744` | LOW | pre-existing |
| P3 | SetupMandate `countryCode` sent by UCS only | `adyen/transformers.rs:6763` | LOW | pre-existing |
| P4 | SetupMandate `applicationInfo` dropped by UCS | `adyen/transformers.rs:6784` | MEDIUM | pre-existing |
| P5 | SetupMandate `merchantOrderReference` sent by UCS only | `adyen/transformers.rs:6777` | LOW | pre-existing |
| P6 | SetupMandate `shopperLocale` sent by UCS only | `adyen/transformers.rs:6768` | LOW | pre-existing |

**Zero response-side mismatches found** — mandate id, network txn id and status extraction are
shared and identical.

---

## 7. Honest scope statement

**Verified:** request-field mapping for Paze and Samsung Pay across Authorize and SetupRecurring;
response-field extraction; mandate/token-id propagation; status mapping; error-field shape;
capability registration — all by reading both implementations line-for-line and running the
validation service's real comparator over the derived payloads.

**Not verified:** anything requiring live paired traffic. No hyperswitch router was run, so **no
genuine router-data pair was ever produced or compared**. The Samsung Pay path has never completed
successfully against Adyen on either implementation (no genuine device SDK token). The absolute
values behind the masked `****` diffs in §5.1 are asserted from source, not observed on the wire.

**Do not read this report as "shadow validation passed."** It did not run. It reports what shadow
validation *would* report, derived statically, and the specific mismatches it would flag.

## 8. Reproduction

```bash
SCRATCH=/tmp/claude-1008/-home-infamous-hyperswitch-prism5/3fd6d774-8f2f-40b4-818b-582555bffa60/scratchpad
cd "$SCRATCH" && git clone --depth 1 https://github.com/juspay/ucs-shadow-validation-service.git
cd ucs-shadow-validation-service/validation-service-headless
npm install --omit=dev --no-audit --no-fund
node run-adyen-diff.js          # §5.1 — driver written by this analysis; inputs derived from source

PORT=9711 REDIS_HOST=127.0.0.1 REDIS_PORT=6379 ENABLE_WEB_DIFF_VIEWER=true \
  IGNORE_KEYS=x-request-id ROUTER_DATA_IGNORE_KEYS=external_latency node server.js &
curl -s -X POST http://localhost:9711/api/router-data \
  -H 'content-type: application/json' -H 'x-flow: router-data' \
  -H 'x-sub-flow: SetupMandate' -H 'x-connector: adyen' \
  -H 'x-request-id: req_paze_setup_001' --data @"$SCRATCH/rd.json"    # §5.2
```

To run the harness for real, the gaps to close are: add this user to the `docker` group; free
enough disk to build `hyperswitch5` (`crates/router`); add `[proxy]` + `mitm_ca_cert` to both
`config/development.toml` files and a `comparison_service` entry on the hyperswitch side; add
`adyen` to `ucs_only_connectors`; seed a merchant + Adyen MCA; set `ucs_enabled` and
`ucs_rollout_config_<merchant>_adyen_wallet_{Authorize,SetupMandate}_shadow`; and obtain a genuine
Samsung Pay device SDK token.
