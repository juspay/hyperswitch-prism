# Pattern Authoring Specification

## Purpose

This specification is the single source of truth for the structural shape of every markdown file under `grace/rulesbook/codegen/guides/patterns/`. Wave-2 through Wave-7 author agents, and every future author, MUST follow it so that ~30 new and ~10 refreshed patterns are structurally identical and machine-reviewable. The spec exists to prevent drift during parallel authoring and to give the Wave-8 reviewer a fixed rubric. It codifies only structure and citation discipline observed in the canonical patterns; content-level correctness is the author's responsibility but will be checked against the pinned connector-service SHA.

The shape mandated here was extracted by reading five flow patterns (`pattern_capture.md`, `pattern_refund.md`, `pattern_psync.md`, `pattern_authorize.md`, `pattern_void.md`, `pattern_rsync.md`) and four PM patterns (`authorize/card/pattern_authorize_card.md`, `authorize/wallet/pattern_authorize_wallet.md`, `authorize/bank_debit/pattern_authorize_bank_debit.md`, and scanning others). Any section required below appeared in at least four of the five flow patterns or is explicitly dictated by the Wave-1 task brief.

## When this spec applies

- Authoring a new flow pattern at the top level (e.g. `pattern_<flow>.md`).
- Authoring a new payment-method pattern under `authorize/<pm>/` (e.g. `authorize/upi/pattern_authorize_upi.md`).
- Refreshing an existing pattern where sections are missing, outdated, or cite retired types.
- Updating PMT-variant enumeration in a PM pattern after `payment_method_data.rs` changes at a new pinned SHA.
- Writing a sub-pattern that qualifies an existing PM pattern (e.g. `pattern_authorize_card_ntid.md`).

Not in scope: README indexes, macro-reference docs, utility reference docs. Those have their own structures and are not bound by this spec.

## Required Sections (Flow Pattern)

A flow pattern (e.g. `pattern_authorize.md`, `pattern_capture.md`, `pattern_refund.md`, `pattern_psync.md`, `pattern_void.md`, `pattern_rsync.md`) MUST contain the following top-level `##` sections in this exact order. Authors MAY add extra sections between them but MAY NOT reorder or omit required ones.

1. `# <Flow> Flow Pattern` — H1 title.
2. `## Overview` — what the flow does in 2-5 sentences, plus a "Key Components" bullet list.
3. `## Table of Contents` — numbered list linking to every `##` section that follows.
4. `## Architecture Overview` — includes a Flow Hierarchy tree (ASCII diagram or bullet tree) and the following Core Types subsections (each as `###`):
   - `### Flow Type` — the marker from `domain_types::connector_flow`.
   - `### Request Type` — the request-data struct from `connector_types` (e.g. `PaymentsAuthorizeData<T>`).
   - `### Response Type` — the response-data struct (e.g. `PaymentsResponseData`).
   - `### Resource Common Data` — the flow-data struct (e.g. `PaymentFlowData`, `RefundFlowData`).
5. `## Connectors with Full Implementation` — a table (see §10) enumerating every connector observed in source that fully implements this flow. Stub/trait-only connectors MUST be listed in a separate sub-table with a "stub" label.
6. `## Common Implementation Patterns` — macro-based pattern first (the recommended path), then alternates (manual implementation, dual-endpoint, etc.).
7. `## Connector-Specific Patterns` — per-connector deviations keyed by connector name; each entry MUST cite `crates/integrations/connector-integration/src/connectors/<name>/...`.
8. `## Code Examples` — real excerpts from the pinned SHA, each with a file-and-line citation.
9. `## Integration Guidelines` — ordered steps an implementer follows; numbered list, no prose-only.
10. `## Best Practices` — bullet list; each bullet either cites a real connector or references another pattern.
11. `## Common Errors / Gotchas` — numbered pitfalls with "Problem" and "Solution" sub-bullets.
12. `## Testing Notes` — unit-test shape, integration-test scenarios table.
13. `## Cross-References` — see §13.

## Required Sections (PM Pattern)

A PM pattern (`authorize/<pm_snake>/pattern_authorize_<pm_snake>.md`) MUST contain, in order:

1. `# <PM> Authorize Flow Pattern` — H1 title.
2. `## Overview` — purpose of the payment method in 2-5 sentences plus a "Key Characteristics" table.
3. `## Variant Enumeration` — see §9. REQUIRED even if the payment method is a single-variant struct.
4. `## Architecture Overview` — types involved, where the variant is unwrapped from `PaymentMethodData<T>`.
5. `## Connectors with Full Implementation` — table per §10, restricted to connectors that actually implement this PM in Authorize.
6. `## Per-Variant Implementation Notes` — one `###` subsection per enum variant, describing the expected transformer path and any connector-specific quirk, with citations.
7. `## Common Implementation Patterns` — shared transformer/matching patterns across connectors.
8. `## Code Examples` — real excerpts with citations.
9. `## Best Practices` — bullet list with citations.
10. `## Common Errors` — Problem/Solution pitfalls.
11. `## Cross-References` — see §13.

Sub-patterns (e.g. `pattern_authorize_card_ntid.md`) follow the PM Pattern shape but MAY merge Variant Enumeration into Overview if they qualify a single variant.

## Canonical Type Signatures

All new patterns MUST reference these canonical signatures verbatim. Do not invent alternate shapes.

```rust
// Generic router-data template (from domain_types::router_data_v2)
RouterDataV2<FlowMarker, FlowData, RequestData, ResponseData>

// Canonical flow-data types (resource_common_data)
PaymentFlowData   // crates/types-traits/domain_types/src/connector_types.rs:796
RefundFlowData    // crates/types-traits/domain_types/src/connector_types.rs:2767
DisputeFlowData   // crates/types-traits/domain_types/src/connector_types.rs:3863
PayoutFlowData    // crates/types-traits/domain_types/src/payouts/payouts_types.rs:17

// Canonical request/response pairs
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
RouterDataV2<Capture,   PaymentFlowData, PaymentsCaptureData,      PaymentsResponseData>
RouterDataV2<Void,      PaymentFlowData, PaymentVoidData,          PaymentsResponseData>
RouterDataV2<PSync,     PaymentFlowData, PaymentsSyncData,         PaymentsResponseData>
RouterDataV2<Refund,    RefundFlowData,  RefundsData,              RefundsResponseData>
RouterDataV2<RSync,     RefundFlowData,  RefundSyncData,           RefundsResponseData>
RouterDataV2<SetupMandate,  PaymentFlowData, SetupMandateRequestData<T>, PaymentsResponseData>  // generic: connector_types.rs:3608
RouterDataV2<CreateOrder,   PaymentFlowData, PaymentCreateOrderData,  PaymentCreateOrderResponse>

// Trait connectors implement
ConnectorIntegrationV2<Flow, FlowData, RequestData, ResponseData>
// from interfaces::connector_integration_v2::ConnectorIntegrationV2

// Auth and error hooks — copy these signatures verbatim.
// interfaces/src/api.rs:25
fn get_auth_header(
    &self,
    _auth_type: &ConnectorSpecificConfig,
) -> CustomResult<Vec<(String, hyperswitch_masking::Maskable<String>)>, IntegrationError>;

// interfaces/src/api.rs:50 — THREE parameters besides &self
fn build_error_response(
    &self,
    res: domain_types::router_response_types::Response,
    _event_builder: Option<&mut events::Event>,   // `events::Event`, NOT `ConnectorEvent`
    _connector_config: &ConnectorSpecificConfig,
) -> CustomResult<ErrorResponse, ConnectorError>;
// Same third parameter on `get_error_response_v2` (connector_integration_v2.rs:187) and
// `get_5xx_error_response` (connector_integration_v2.rs:200).

// interfaces/src/verification.rs:20 and interfaces/src/decode.rs:6 — NON-GENERIC.
// Exactly ONE impl of each per connector; never one per flow (a
// `SourceVerification<Flow, Data, Req, Resp>` impl is E0107).
// Exemplar: connectors/travelhub.rs:174-183
pub trait SourceVerification { /* .. */ }
pub trait BodyDecoding { /* .. */ }
```

`RouterDataV2` fields (`domain_types/src/router_data_v2.rs:6`): `flow`, `resource_common_data`,
`connector_config: ConnectorSpecificConfig`, `request`, `response`. There is **no**
`connector_auth_type` and **no** `connector_meta_data` field — both were removed
(`a7a696c3a`, 2026-03-14). Per-flow metadata lives on the request or on
`resource_common_data`.

PM patterns MUST use the `PaymentsAuthorizeData<T>` form (generic `T: PaymentMethodDataTypes`). Patterns MUST NOT reference RouterData (V1).

## Code-Citation Rules

Every factual claim about a connector's behavior, type layout, or URL scheme MUST be backed by one of:

1. A file path and line number, `path/to/file.rs:<line>`, rendered in backticks.
2. A fenced code block with a comment header of the form `// From <path>:<line>`.

The pinned SHA is the reference tree; line numbers MUST resolve at that SHA. Authors MUST NOT use the words "likely", "probably", "typically", "usually", "often", "most connectors" unless the sentence containing them ends with a citation that substantiates the claim. Statements of the form "Connector X does Y" without a citation are a reviewer FAIL.

## Variant-Enumeration Rule (PM patterns)

A PM pattern MUST enumerate every variant of the corresponding enum in `crates/types-traits/domain_types/src/payment_method_data.rs` at the pinned SHA. The mapping is:

| PM directory | Enum |
|--------------|------|
| `authorize/card/` | `Card<T>`, `CardToken`, `NetworkTokenData`, `CardDetailsForNetworkTransactionId` |
| `authorize/wallet/` | `WalletData` |
| `authorize/bank_debit/` | `BankDebitData` |
| `authorize/bank_transfer/` | `BankTransferData` |
| `authorize/bank_redirect/` | `BankRedirectData` |
| `authorize/upi/` | `UpiData` |
| `authorize/bnpl/` | `PayLaterData` |
| `authorize/crypto/` | `CryptoData` |
| `authorize/gift_card/` | `GiftCardData` |
| `authorize/mobile_payment/` | `MobilePaymentData` |
| `authorize/reward/` | `RewardData` (if present at pinned SHA; otherwise note absent) |

The Variant Enumeration section MUST be a table with columns: Variant | Data Shape | Citation | Used By (connectors). The reviewer will diff the listed variants against the enum's variants at the pinned SHA. A missing variant is an automatic FAIL. If a variant has no connector implementation, the "Used By" cell MUST say "(none)" rather than being omitted.

## Connectors-with-Full-Implementation table

Required columns, in this order:

| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |

Rules:
- Rows MUST list only connectors observed in `crates/integrations/connector-integration/src/connectors/` at the pinned SHA. No speculative entries.
- "Request Type Reuse" names the connector-local request struct (e.g. `AdyenCaptureRequest`) and notes whether it is reused for other flows (e.g. "reuses `AdyenPaymentRequest`").
- Stub-only implementations MUST NOT appear in this table. Put them under a sibling `### Stub Implementations` sub-section with a single-column list.
- Ordering: alphabetical by connector.

## Banned anti-patterns

Authors MUST NOT:

1. Hardcode statuses inside transformer `TryFrom` blocks (e.g. `status: AttemptStatus::Charged` literal). Map from the connector response instead.
2. Mock databases or HTTP layers inside documented integration tests. Integration tests in patterns MUST describe real sandbox flows.
3. Reference retired types. In particular:
   - Treating `ConnectorError` as the request-side error type. The error enum was split: `IntegrationError` (`errors.rs:115`) covers request-building and validation, `ConnectorError` (`errors.rs:371`) covers the response side ONLY and has exactly five variants — `ResponseDeserializationFailed`, `ResponseHandlingFailed`, `UnexpectedResponseError`, `IntegrityCheckFailed`, `ConnectorErrorResponse`. There is no `ConnectorResponseTransformationError` type.
   - `RouterDataV2::connector_auth_type`. That field was removed on 2026-03-14 (`a7a696c3a`); auth now travels as `connector_config: ConnectorSpecificConfig` (`router_data_v2.rs:14`), whose variants are per-connector (`router_data.rs:301`).
4. Duplicate utility-function bodies inline. Link to `utility_functions_reference.md` and call the function instead.
5. Emit handwavy prose ("this usually works", "most connectors do X") without a citation per §8.
6. Silently omit enum variants in PM patterns. Every variant is accounted for or the pattern FAILs review.
7. `unwrap_or_default()` on `ErrorResponse.code` or `ErrorResponse.message`. Use
   `.unwrap_or_else(|| NO_ERROR_CODE.to_string())` / `NO_ERROR_MESSAGE`
   (`crates/common/common_utils/src/consts.rs:154-156`). Real connectors reference these consts 497 times across 90 files (`grep -roh 'NO_ERROR_CODE\|NO_ERROR_MESSAGE' crates/integrations/connector-integration/src/connectors/ | wc -l`);
   an empty string in a log is indistinguishable from "the connector sent nothing".
8. Force a terminal status on a shared error path. `ErrorResponse.attempt_status` is
   `Option<FlowStatus>` (`domain_types/src/router_data.rs:4233`), and `FlowStatus`
   (`router_data.rs:4186`) is flow-aware: `Payment(AttemptStatus)`, `Refund(RefundStatus)`,
   `Dispute(DisputeStatus)`, `Payout(PayoutStatus)`. Hardcoding
   `Some(FlowStatus::Payment(AttemptStatus::Failure))` on the shared path is what reports a
   charged payment as FAILURE. A blanket `None` is equally wrong — it leaves a hard-declined
   refund Pending and retrying forever. Map only what the connector's own payload proves, per
   flow. Exemplars: `connectors/flywire.rs:355-371` (flow-aware, picks `Refund` vs `Payment`),
   `connectors/noon.rs:498-512` (minimal form).
9. Catch-all `_ =>` at the STATUS-MAPPING layer. Two halves are required and reviewers check
   both: (a) the connector status enum ends with `#[serde(other)] Unknown` at the
   DESERIALIZATION layer so an unrecognised wire value parses, and (b) the mapping `match` is
   exhaustive over that enum with an explicit `Unknown` arm and no wildcard. Exemplars:
   `TravelhubResult` (`connectors/travelhub/transformers.rs:494-509`) and
   `map_travelhub_status` (`connectors/travelhub/transformers.rs:556-568`), where `Unknown`
   maps to the non-terminal `AttemptStatus::Pending`.
10. Prescribe a default amount unit. "Default to `StringMinorUnit` if unclear" is wrong for
    roughly four connectors in five. Read the vendor spec and match its wire format; the HEAD
    split is `StringMajorUnit` 24, `FloatMajorUnit` 22, `MinorUnit` 11, `StringMinorUnit` 8,
    and a fifth type, `StringTwoDecimalUnit`, exists (`common_utils/src/types.rs:443`).
11. Swallow an in-band 2xx failure. When a connector answers 200 with a failure payload, the
    transformer MUST return `Err(ErrorResponse { .. })`, branching on a success predicate over
    the MAPPED status — not on the presence of an `error` field. Reference predicate:
    `domain_types::utils::is_payment_failure` (`domain_types/src/utils.rs:231`).
12. Emit a per-flow `SourceVerification` or `BodyDecoding` impl. Both traits are non-generic
    (`interfaces/src/verification.rs:20`, `interfaces/src/decode.rs:6`); exactly one impl of
    each per connector. A `SourceVerification<Flow, Data, Req, Resp>` impl is E0107.
    Exemplar: `connectors/travelhub.rs:174-183`.
13. Omit `macro_connector_flow_status_impls!`. Every flow the connector does not implement must
    be listed under `not_implemented:` or `not_supported:` (`connectors/macros.rs:1827`); all
    111 connectors on HEAD invoke it (all of them), and without it the connector does not compile.

## Retired types to avoid

The following names MUST NOT appear in any new pattern at this pinned SHA. Occurrences trigger reviewer FAIL unless wrapped in a "retired — do not use" callout.

- `ConnectorError` used for request-time failures. `ConnectorError` (`crates/types-traits/domain_types/src/errors.rs:371`) is the RESPONSE-side enum and has exactly five variants, four of them struct variants requiring a `context: ResponseTransformationErrorContext`. Request-time failures use `IntegrationError` (`errors.rs:115`). `ConnectorResponseTransformationError` does not exist and MUST NOT be written.
- `RouterData` (V1). Replace with `RouterDataV2<...>`.
- `ApiErrorResponse` legacy shape, if referenced. Use `ErrorResponse` from `domain_types::router_data`.
- `RouterDataV2::connector_auth_type` and `get_auth_header(&ConnectorAuthType)`. The field is gone (removed 2026-03-14, `a7a696c3a`) and the trait method's parameter is `&ConnectorSpecificConfig` (`interfaces/src/api.rs:25`); an impl still taking `&ConnectorAuthType` no longer matches the trait (E0407). Read auth from `req.connector_config` and match your connector's own `ConnectorSpecificConfig` variant (`domain_types/src/router_data.rs:301`). Exemplars: `connectors/travelhub/transformers.rs:46`, `connectors/volt/transformers.rs:398`.
- Any pre-rename auth-token type from before PR #855 (commit `c9e1025e3`, 2026-04-02). The full rename map is:
  - flow-marker structs in `connector_flow.rs`: `CreateSessionToken` → `ServerSessionAuthenticationToken`; `CreateAccessToken` → `ServerAuthenticationToken`; `SdkSessionToken` → `ClientAuthenticationToken`.
  - traits in `interfaces/src/connector_types.rs`: `PaymentSessionToken` → `ServerSessionAuthentication`; `PaymentAccessToken` → `ServerAuthentication`; `SdkSessionTokenV2` → `ClientAuthentication`.
  - request/response data types in `connector_types.rs`: `PaymentsSdkSessionTokenData` → `ClientAuthenticationTokenRequestData`; `SessionTokenRequestData` → `ServerSessionAuthenticationTokenRequestData`; `SessionTokenResponseData` → `ServerSessionAuthenticationTokenResponseData`; `AccessTokenRequestData` → `ServerAuthenticationTokenRequestData`; `AccessTokenResponseData` → `ServerAuthenticationTokenResponseData`.
  - top-level response enum in `connector_types.rs`: `SessionToken` (the sdk-data payload enum, NOT `FlowName::SessionToken`) → `ClientAuthenticationTokenData`.
  Also check `domain_types::router_data::ConnectorSpecificConfig` (`router_data.rs:301`) for your connector's variant and use its field names exactly.
- `api::ConnectorIntegration` (V1 trait). Replace with `interfaces::connector_integration_v2::ConnectorIntegrationV2`.
- Hand-rolled amount conversion helpers. Use the macro-generated amount converter (`macros::create_amount_converter_wrapper!`, `connectors/macros.rs:1377`) and one of the FIVE unit types in `common_utils::types`: `MinorUnit` (`types.rs:170`), `StringMinorUnit` (`types.rs:305`), `FloatMajorUnit` (`types.rs:336`), `StringMajorUnit` (`types.rs:374`), `StringTwoDecimalUnit` (`types.rs:443`). Pick the one that matches the vendor's documented wire format — do NOT default to `StringMinorUnit`; on HEAD the split is `StringMajorUnit` 24, `FloatMajorUnit` 22, `MinorUnit` 11, `StringMinorUnit` 8.

If the author is unsure whether a type is retired, grep `grace/rulesbook/codegen/guides/utility_functions_reference.md` and `grace/rulesbook/codegen/guides/types/types.md` for the current canonical name. Those two files, at the pinned SHA, are authoritative.

## Required Cross-References

Every new pattern's `## Cross-References` section MUST link to:

1. The parent README: `../README.md` for flow patterns; `../../README.md` and `../README.md` for PM patterns (i.e. both the `patterns/` index and the `authorize/` index).
2. At least two sibling patterns in the same category. For flow patterns, that means two other flow patterns (e.g. a `pattern_capture.md` links to `pattern_authorize.md` and `pattern_void.md`). For PM patterns, that means two other PM patterns (e.g. `pattern_authorize_upi.md` links to `pattern_authorize_card.md` and `pattern_authorize_wallet.md`).
3. `../utility_functions_reference.md` (or the correct relative path) IF the pattern cites any utility helper.
4. `../../types/types.md` (or the correct relative path) whenever the pattern uses a non-obvious type beyond the canonical signatures in §7.

Links MUST be relative markdown links, not absolute paths.

## Review Rubric

The Wave-8 reviewer will run these ten checks in order. Any check failing returns the artifact to its author.

1. **Section order.** The `##` headers MUST appear in the order mandated by §5 (flow) or §6 (PM). Extra sections allowed between required ones; reordering or omission FAILs.
2. **Citations present.** Every non-obvious factual claim is backed per §8. Grep for banned hedge words without accompanying citations.
3. **All enum variants enumerated (PM patterns only).** Variant-Enumeration table variants match the enum at the pinned SHA exactly.
4. **RouterDataV2 params correct.** Every `RouterDataV2<...>` in the pattern has four type arguments drawn from §7; no three-arg forms, no V1 `RouterData`.
5. **No retired types.** Names listed in §12 MUST NOT appear outside a retired-callout.
6. **Cross-refs present.** The four requirements of §13 are satisfied with working relative links.
7. **Code snippets look syntactically plausible.** Rust fences parse as Rust (balanced braces, `impl ... for ...` blocks complete, `use` paths resolve against current crates). The reviewer does not compile them but performs a visual scan.
8. **Trait signatures match HEAD.** Grep the pattern for `build_error_response`,
   `get_error_response_v2`, `get_5xx_error_response` (three parameters besides `&self`, third is
   `&ConnectorSpecificConfig`), `get_auth_header` (`&ConnectorSpecificConfig`),
   `SourceVerification` / `BodyDecoding` (non-generic, one impl per connector), `get_event_type`
   (one argument besides `&self`) and `process_payment_webhook` (four). Any mismatch with §7
   FAILs.
9. **Struct literals list every field.** `ErrorResponse` has 13 fields and an
   `impl Default` (`domain_types/src/router_data.rs:4228,4244`), so `..Default::default()` is
   acceptable. `PaymentsResponseData::TransactionResponse` (11 fields, `connector_types.rs:2009`)
   and the other enum struct-variants have NO functional-update syntax — every omitted field is
   E0063, so they must be listed in full.
10. **No banned anti-pattern from §11 items 7-13.** Grep for `unwrap_or_default()` on an error
    code/message, `attempt_status: Some(AttemptStatus::` on a shared error path, a `_ =>` arm at
    the status-mapping layer, and "default to StringMinorUnit" advice.

A pattern passes only when all ten checks pass.

## File-naming conventions

- Flow patterns at top level: `pattern_<flow_snake>.md`. Examples: `pattern_authorize.md`, `pattern_capture.md`, `pattern_refund.md`, `pattern_psync.md`, `pattern_void.md`, `pattern_rsync.md`, `pattern_setup_mandate.md`, `pattern_repeat_payment.md`, `pattern_incoming_webhook.md`.
- PM patterns under `authorize/`: `authorize/<pm_snake>/pattern_authorize_<pm_snake>.md`. Examples: `authorize/card/pattern_authorize_card.md`, `authorize/wallet/pattern_authorize_wallet.md`, `authorize/bank_debit/pattern_authorize_bank_debit.md`.
- Sub-patterns (qualified variants of a PM pattern): `authorize/<pm_snake>/pattern_authorize_<pm_snake>_<qualifier_snake>.md`. Examples: `authorize/card/pattern_authorize_card_ntid.md`, `authorize/card/pattern_authorize_card_3ds.md`.
- All filenames MUST be lowercase snake_case. No camelCase, no hyphens, no spaces.
- Directory names match their pattern's `<pm_snake>` exactly.

## Failure → revision loop

A FAIL verdict from the Wave-8 reviewer returns the artifact to its originating author agent with a structured list of failed rubric checks (§14). The author performs one revision cycle and resubmits. If the second submission also FAILs, the artifact escalates to human review rather than entering a third autonomous revision. The reviewer MUST cite rubric-check numbers (1-10) when failing; the author MUST address each cited check in the revision. Revision commits MUST preserve the file path; authors do not rename a pattern during revision. Escalated artifacts block their downstream waves until a human reviewer resolves them; the orchestrator is responsible for re-queueing the PR after human approval.

## Worked example: minimal conforming flow pattern skeleton

Authors MAY copy this skeleton as a starting point. Replace bracketed placeholders and add real citations.

```markdown
# <Flow> Flow Pattern

## Overview
Two-to-five-sentence description. Key Components:
- Main connector file: ...
- Transformers file: ...

## Table of Contents
1. [Architecture Overview](#architecture-overview)
2. [Connectors with Full Implementation](#connectors-with-full-implementation)
... (one entry per remaining ## section)

## Architecture Overview
### Flow Type
`<Flow>` marker, from `domain_types::connector_flow`.
### Request Type
`<RequestData>` — see `crates/types-traits/domain_types/src/connector_types.rs:<line>`.
### Response Type
`<ResponseData>` — see ...
### Resource Common Data
`<FlowData>` — see ...

## Connectors with Full Implementation
| Connector | HTTP Method | Content Type | URL Pattern | Request Type Reuse | Notes |
| --- | --- | --- | --- | --- | --- |
| Adyen | POST | application/json | /v68/.../<flow> | AdyenCaptureRequest (reuses AdyenPaymentRequest shape) | See crates/.../adyen/transformers.rs:<line> |

### Stub Implementations
- <connector list>

## Common Implementation Patterns
...

## Connector-Specific Patterns
...

## Code Examples
...

## Integration Guidelines
1. ...

## Best Practices
- ...

## Common Errors / Gotchas
1. Problem: ... Solution: ...

## Testing Notes
...

## Cross-References
- Parent index: [../README.md](../README.md)
- Sibling flow: [pattern_authorize.md](./pattern_authorize.md)
- Sibling flow: [pattern_void.md](./pattern_void.md)
- Types: [../types/types.md](../types/types.md)
```

The PM-pattern skeleton differs by replacing "Connectors with Full Implementation" with the per-PM table and inserting the mandatory "Variant Enumeration" table immediately after Overview.
