# TSYS TransIT (XML) Connector — UCS + Hyperswitch Design

**Date:** 2026-05-14
**Owner:** Malay Awasthi
**Status:** Draft — pending implementation
**Cert kit:** `~/Downloads/TransIT Script v6.2 Case 00233192 Juspay inc (TAS) (1).xlsx`
**API docs:** https://developerportal.transit-pass.com/developerportal/resources/dist/#/api-specs
**Target:** First cert test batch to TSYS in the week of 2026-05-19

---

## 1. Goal

Add a new sibling connector `tsys_xml` (`TsysXml`) to connector-service and route it through Hyperswitch as a **UCS-only connector**. The integration targets the TSYS TransIT XML API (`API3.0`) and certifies against the TransIT Script v6.2 across three card-data-source contexts:

- Direct Marketing — **PHONE only** (no MAIL)
- E-commerce (INTERNET)
- Recurring / Installments

The existing legacy JSON `tsys` connector is left untouched.

## 2. Non-goals

- Replace or modify the existing JSON `tsys` connector.
- Implement MAIL cardDataSource flows.
- Implement `GenerateKey` as a runtime flow. The `transactionKey` is long-lived per MID (per cert script note) and is provisioned out-of-band by ops, then stored in the merchant's `ConnectorAuthType`.
- Implement settlement triggering — TSYS settles automatically every hour at :30.

## 3. Repo & branch strategy

| Repo | Cut from | New branch |
|---|---|---|
| `connector-service` (UCS) | `origin/main` (after `git pull --ff-only`) | `feat/tsys_xml-transit-connector` |
| `hyperswitch` | `origin/main` (after switching off the unrelated `signified-fix` WIP branch and pulling main) | `feat/tsys_xml-ucs-routing` |

The HS local branch `signified-fix` (WIP, diverged from `fork/signifyd-pre-frm-fixes`) is **not touched**; we cut a fresh branch from `origin/main`. The local untracked `.claude/` folder stays untracked.

## 4. UCS layout

Modelled on `worldpayxml` (the canonical XML reference in-repo):

```
crates/integrations/connector-integration/src/connectors/
├── tsys_xml.rs                 ← Connector struct + ConnectorIntegrationV2 impls
└── tsys_xml/
    ├── requests.rs             ← XML request types (serde + quick_xml::se)
    ├── responses.rs            ← XML response types (serde + quick_xml::de)
    └── transformers.rs         ← RouterData ↔ XML request/response mapping
```

Plus wiring:

- `connectors/mod.rs` — register module + `pub use tsys_xml::TsysXml`
- Connector enum (UCS-side, in `crates/integrations/connector-integration` and `crates/types-traits`): add `TsysXml` variant alongside existing `Tsys`
- `Cargo.toml` — no new deps (`quick_xml` already in tree)
- `config/development.toml`, `config/docker_compose.toml` (UCS-side): `tsys_xml.base_url = "https://stagegwapi.transit-pass.com/"` (exact host to be confirmed from cert kit)

**Why mirror `worldpayxml`**: it already wires the XML envelope helper (`generate_soap_xml`-style), the `GetSoapXml` trait, `quick_xml::se::to_string`, and the `BodyDecoding` impl for response parsing. Same machinery TSYS TransIT needs.

## 5. Authentication

TransIT XML carries auth **inside the request body**, not in headers. The auth fields are flattened directly into the operation root element (NO `<MerchantAuthentication>` envelope):

```xml
<Sale>
  <deviceID>{{device_id}}</deviceID>
  <transactionKey>{{transaction_key}}</transactionKey>
  <developerID>{{developer_id}}</developerID>
  <cardDataSource>PHONE</cardDataSource>
  …
</Sale>
```

Exact field casing (mandatory): `deviceID`, `transactionKey`, `developerID` (capital `ID`, lowercase `transactionKey`).

**ConnectorAuthType mapping** (long-lived, per-merchant):

```rust
ConnectorAuthType::SignatureKey {
    api_key:    Secret<String>,   // → transactionKey  (long-lived per MID)
    key1:       String,           // → deviceID        (per-MID device id)
    api_secret: Secret<String>,   // → developerID     (Juspay's TransIT dev id)
}
```

HTTP transport: `Content-Type: text/xml`, `POST` to a single TransIT endpoint per env. No HTTP-layer auth headers.

## 6. Flows & XML operation mapping

| HS / UCS flow | TransIT XML root | Notes |
|---|---|---|
| `Authorize` (auto-capture) | `<Sale>` | Auth + capture in one call |
| `Authorize` (manual capture) | `<Auth>` | Auth only |
| `Capture` | `<Capture>` | Supports multi-clearing: `seqNumber` + `paymentCount` for split shipment |
| `IncrementalAuthorization` | `<Auth>` referencing original `transactionID` | MOTO Step 5 |
| `Void` (full) | `<Void>` | Pre-settlement cancel |
| `Void` (partial) | `<Void>` with `transactionAmount` | Cert script Step 7 — supported |
| `Refund` (referenced, full) | `<Return>` w/ original `transactionID` | Post-settlement |
| `Refund` (referenced, partial) | `<Return>` w/ `transactionAmount` | Post-settlement |
| `Refund` (unreferenced) | `<Return>` with full card data, no original ref | "Return WITHOUT Reference" |
| Return reversal | `<Void>` referencing a `<Return>` | Step 11/13 |
| `PSync` | `<TransactionInquiry>` *(name to confirm with TSYS)* | Open question #5 below |
| Card verify (0-dollar) | `<CardAuthentication>` | Step 3 in both sheets |
| **Recurring vault setup** | `<AddCustomer>` → returns `customerCode` + `walletID` | Sheet 5 Step 2 |
| **Recurring schedule** | `<AddRecurringSchedule>` referencing `customerCode` + `walletID` | Sheet 5 Step 3 |

**Cross-cutting concerns** (carried as extra XML fields, not separate flows):

- **3DS / UCAF** (ecomm Step 8): `eciIndicator` ∈ {5, 6, 7}, `ucafCollectionIndicator`, `cavv`, `xid`, `dsTransactionID`.
- **CIT / MIT / COF indicators** (MOTO Step 5, ecomm Step 9): `cardOnFile`, `mitIndicator`, network transaction id reuse.
- **Level 2 / Level 3 enhanced data** (Step 4 in both sheets): `salesTax`, `purchaseOrder`, `commercialCardLevel`, and a list of `<productDetails>`.
- **Specialised Sales** (BillPay / Debt Repayment / Account Funding): Sale with a transaction-category flag.
- **Offline / Force** (MOTO Step 12 — "Support Required"): `<Force>` with TSYS-issued approval code. Opt-in; flag for ops handover.

**Card data source mapping per cert tab:**

| Cert tab | `cardDataSource` |
|---|---|
| Direct Marketing — Phone | `PHONE` |
| E-commerce | `INTERNET` |
| Recurring CIT (first use) | `INTERNET` + COF indicators |
| Recurring MIT (subsequent) | `RECURRING` *(value to confirm)* + `customerCode`/`walletID` |
| Incremental Auth | `MANUAL` |

## 7. XML serde patterns

Pulled directly from the `worldpayxml/requests.rs` recipe:

```rust
#[derive(Debug, Serialize)]
#[serde(rename = "Sale")]
pub struct TsysXmlSaleRequest {
    #[serde(rename = "deviceID")]      pub device_id: Secret<String>,
    #[serde(rename = "transactionKey")] pub transaction_key: Secret<String>,
    #[serde(rename = "developerID")]    pub developer_id: Secret<String>,
    #[serde(rename = "cardDataSource")] pub card_data_source: CardDataSource,
    #[serde(rename = "transactionAmount")] pub transaction_amount: StringMinorUnit,
    #[serde(rename = "cardNumber")]     pub card_number: Secret<String>,
    #[serde(rename = "expirationDate")] pub expiration_date: Secret<String>,
    #[serde(rename = "cvv2")]           pub cvv2: Option<Secret<String>>,
    // … L2/L3, 3DS, COF, multi-clearing, BillPay fields optional, gated by flow type
}
```

- One root struct per `transactionType` (operation IS the root element — no shared envelope).
- Serialise with `quick_xml::se::to_string`, prepend `<?xml version="1.0" encoding="UTF-8"?>\n`.
- Field-by-field `#[serde(rename = "…")]` to lock TransIT's mixed camelCase / capital-`ID` casing exactly.
- Response deserialisation via `quick_xml::de::from_str` inside the `BodyDecoding` impl, response root is `<*Response>` (e.g. `<SaleResponse>`).

## 8. Hyperswitch-side delta

`tsys_xml` is **UCS-only**: HS does not compile a connector implementation for it. The router dispatches via gRPC.

1. **Connector enum**: add `TsysXml` variant in `api_models::enums::Connector`, `common_enums::connector_enums::Connector`, and `router::types::transformers` mappings; wire `ToString` / `FromStr` / `serde` (`tsys_xml` ↔ `TsysXml`).
2. **Config**: add `tsys_xml` to `ucs_only_connectors` in:
   - `config/config.example.toml`
   - `config/development.toml`
   - `config/docker_compose.toml`
   - Add `tsys_xml.base_url = "…"` block alongside the existing `tsys.base_url`.
   - Add `[pm_filters.tsys_xml]` (cards only — Visa, MasterCard, Amex, Discover, Diners, JCB; matches script).
3. **No `hyperswitch_connectors/src/connectors/tsys_xml.rs`** — UCS-only routing means no HS-side connector code.
4. **`connector_meta_data` schema**: no new fields required for PR 1 — auth carries everything, and per-transaction signals (eciIndicator, COF indicators) flow through existing payment request fields.
5. **Cypress tests**: a `tsys_xml/` test set mirroring the existing `tsys/` Cypress tests. Optional for PR 1 if not blocking cert handover, but recommended.

## 9. Test plan

- **UCS unit tests** in `tsys_xml/transformers.rs` `#[cfg(test)]`:
  - Per-operation round-trip golden tests — assert generated XML byte-for-byte matches the cert kit's expected request XML (one per script row in the three relevant sheets).
  - Response parsing: feed canned `<*Response>` XML and assert `RouterData` mapping.
- **UCS integration tests**: stub the TransIT host with `wiremock`, exercise each `transactionType` happy-path + at least one decline.
- **Cert mapping spreadsheet → tests**: each row in MOTO / e-Commerce / Recurring sheets gets a corresponding `#[test]` case, named after the row label (e.g. `motoPhone_visaLevel2_approved`).
- **HS-side**: connector_enum unit tests + a Cypress e2e suite mirroring `tsys`'s.

## 10. Open questions to TSYS (for the 7PM call)

Most original open questions are already answered by the cert script. Remaining items to confirm on the call:

1. ✅ **Void before/after capture?** — Both. Pre-settlement → `<Void>`. Post-settlement → `<Return>`. (Confirmed via script Step 7 + Step 10.)
2. ✅ **Batch settlement** — Fully automatic on TSYS side, hourly at :30. No merchant action needed.
3. ❌ **Partial void NOT supported?** — Wrong assumption. Script Step 7 includes Partial Void tests; **must implement**.
4. ✅ **L2/L3 required?** — Yes, mandatory per cert script Step 4 in both sheets.
5. ❓ **PSync XML root element name** — Cert script doesn't include a TransactionInquiry / status-lookup case. Need to confirm `<TransactionInquiry>` vs `<GetDetails>` vs another name with TSYS.
6. ❓ **`cardDataSource` value for Recurring MIT** — Script names the tab "Recurring" but the per-call `cardDataSource` value for subsequent (merchant-initiated) recurring charges isn't shown explicitly. Confirm with TSYS.
7. ❓ **Offline / Force** ("Support Required" in script Step 12) — Need TSYS to confirm enablement on our MID and to share approval-code semantics; otherwise we feature-gate behind a config and exclude from cert PR.

## 11. Scope flag / risk

Total surface area for cert PR 1: ~12 transaction types × 3 contexts + 6 cross-cutting concerns (3DS, COF, L3, multi-clearing, recurring vault, incremental auth). This is realistically 2–3 weeks of focused engineering, not days. Recommend the team flag this honestly with the TSYS counterpart on the 7PM call before committing to a date for the first cert submission.

## 12. Out of scope for PR 1 (deferred)

- `GenerateKey` runtime flow — credentials are minted out-of-band by ops.
- MAIL cardDataSource.
- `Offline / Force` (`<Force>`) — pending TSYS enablement confirmation.
- Account Funding / Debt Repayment / BillPay specialised flags — included **only if** required by cert script handover (script lists them, will revisit).
- Webhook / async notification — TransIT is sync request/response only; nothing to wire.

## 13. Implementation order (handoff to writing-plans)

1. Skeleton: empty `tsys_xml.rs` + `tsys_xml/{requests,responses,transformers}.rs`, register module, enum variant, build green.
2. Auth + HTTP plumbing + base URL config (UCS + HS).
3. `<Sale>` (auto-capture) — minimal happy path, PHONE + INTERNET.
4. `<Auth>` + `<Capture>` (single).
5. `<Void>` (full + partial) and `<Return>` (referenced full + partial, unreferenced).
6. `<CardAuthentication>` zero-dollar verify.
7. L2 / L3 fields (Step 4 in both sheets).
8. 3DS / UCAF fields (ecomm Step 8).
9. COF / CIT / MIT indicators (MOTO Step 5 / ecomm Step 9).
10. Multi-clearing captures (`seqNumber` / `paymentCount`).
11. Incremental Auth.
12. Recurring vault: `<AddCustomer>` + `<AddRecurringSchedule>`.
13. `<TransactionInquiry>` for PSync (after TSYS confirms name).
14. HS-side enum + config wiring + Cypress.
