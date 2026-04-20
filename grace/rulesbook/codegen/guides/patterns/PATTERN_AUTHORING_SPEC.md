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
PaymentFlowData   // crates/types-traits/domain_types/src/connector_types.rs:422
RefundFlowData    // crates/types-traits/domain_types/src/connector_types.rs:1779
DisputeFlowData   // crates/types-traits/domain_types/src/connector_types.rs:2648
PayoutFlowData    // crates/types-traits/domain_types/src/payouts/payouts_types.rs:13

// Canonical request/response pairs
RouterDataV2<Authorize, PaymentFlowData, PaymentsAuthorizeData<T>, PaymentsResponseData>
RouterDataV2<Capture,   PaymentFlowData, PaymentsCaptureData,      PaymentsResponseData>
RouterDataV2<Void,      PaymentFlowData, PaymentVoidData,          PaymentsResponseData>
RouterDataV2<PSync,     PaymentFlowData, PaymentsSyncData,         PaymentsResponseData>
RouterDataV2<Refund,    RefundFlowData,  RefundsData,              RefundsResponseData>
RouterDataV2<RSync,     RefundFlowData,  RefundSyncData,           RefundsResponseData>
RouterDataV2<SetupMandate,  PaymentFlowData, SetupMandateRequestData, PaymentsResponseData>
RouterDataV2<CreateOrder,   PaymentFlowData, PaymentCreateOrderData,  PaymentCreateOrderResponse>

// Trait connectors implement
ConnectorIntegrationV2<Flow, FlowData, RequestData, ResponseData>
// from interfaces::connector_integration_v2::ConnectorIntegrationV2
```

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
   - The pre-split monolithic `ConnectorError` (replaced per PR #765 by `IntegrationError` and `ConnectorResponseTransformationError`).
   - Pre-rename auth-token types replaced per PR #855. Use the current `ConnectorAuthType` variants as defined in `domain_types::router_data`.
4. Duplicate utility-function bodies inline. Link to `utility_functions_reference.md` and call the function instead.
5. Emit handwavy prose ("this usually works", "most connectors do X") without a citation per §8.
6. Silently omit enum variants in PM patterns. Every variant is accounted for or the pattern FAILs review.

## Retired types to avoid

The following names MUST NOT appear in any new pattern at this pinned SHA. Occurrences trigger reviewer FAIL unless wrapped in a "retired — do not use" callout.

- `ConnectorError` (monolithic, pre-PR-#765). Replace with `IntegrationError` (request-time) or `ConnectorResponseTransformationError` (response-parse-time).
- `RouterData` (V1). Replace with `RouterDataV2<...>`.
- `ApiErrorResponse` legacy shape, if referenced. Use `ErrorResponse` from `domain_types::router_data`.
- Any pre-rename auth-token type from before PR #855 (commit `c9e1025e3`, 2026-04-02). The full rename map is:
  - flow-marker structs in `connector_flow.rs`: `CreateSessionToken` → `ServerSessionAuthenticationToken`; `CreateAccessToken` → `ServerAuthenticationToken`; `SdkSessionToken` → `ClientAuthenticationToken`.
  - traits in `interfaces/src/connector_types.rs`: `PaymentSessionToken` → `ServerSessionAuthentication`; `PaymentAccessToken` → `ServerAuthentication`; `SdkSessionTokenV2` → `ClientAuthentication`.
  - request/response data types in `connector_types.rs`: `PaymentsSdkSessionTokenData` → `ClientAuthenticationTokenRequestData`; `SessionTokenRequestData` → `ServerSessionAuthenticationTokenRequestData`; `SessionTokenResponseData` → `ServerSessionAuthenticationTokenResponseData`; `AccessTokenRequestData` → `ServerAuthenticationTokenRequestData`; `AccessTokenResponseData` → `ServerAuthenticationTokenResponseData`.
  - top-level response enum in `connector_types.rs`: `SessionToken` (the sdk-data payload enum, NOT `FlowName::SessionToken`) → `ClientAuthenticationTokenData`.
  Also check `domain_types::router_data::ConnectorAuthType` for the current variant set and use those names exactly.
- `api::ConnectorIntegration` (V1 trait). Replace with `interfaces::connector_integration_v2::ConnectorIntegrationV2`.
- Hand-rolled amount conversion helpers. Use the macro-generated amount converter (`macros::create_amount_converter_wrapper!`) and types from `common_utils::types`: `MinorUnit`, `StringMinorUnit`, `StringMajorUnit`, `FloatMajorUnit`.

If the author is unsure whether a type is retired, grep `grace/rulesbook/codegen/guides/utility_functions_reference.md` and `grace/rulesbook/codegen/guides/types/types.md` for the current canonical name. Those two files, at the pinned SHA, are authoritative.

## Required Cross-References

Every new pattern's `## Cross-References` section MUST link to:

1. The parent README: `../README.md` for flow patterns; `../../README.md` and `../README.md` for PM patterns (i.e. both the `patterns/` index and the `authorize/` index).
2. At least two sibling patterns in the same category. For flow patterns, that means two other flow patterns (e.g. a `pattern_capture.md` links to `pattern_authorize.md` and `pattern_void.md`). For PM patterns, that means two other PM patterns (e.g. `pattern_authorize_upi.md` links to `pattern_authorize_card.md` and `pattern_authorize_wallet.md`).
3. `../utility_functions_reference.md` (or the correct relative path) IF the pattern cites any utility helper.
4. `../../types/types.md` (or the correct relative path) whenever the pattern uses a non-obvious type beyond the canonical signatures in §7.

Links MUST be relative markdown links, not absolute paths.

## Review Rubric

The Wave-8 reviewer will run these seven checks in order. Any check failing returns the artifact to its author.

1. **Section order.** The `##` headers MUST appear in the order mandated by §5 (flow) or §6 (PM). Extra sections allowed between required ones; reordering or omission FAILs.
2. **Citations present.** Every non-obvious factual claim is backed per §8. Grep for banned hedge words without accompanying citations.
3. **All enum variants enumerated (PM patterns only).** Variant-Enumeration table variants match the enum at the pinned SHA exactly.
4. **RouterDataV2 params correct.** Every `RouterDataV2<...>` in the pattern has four type arguments drawn from §7; no three-arg forms, no V1 `RouterData`.
5. **No retired types.** Names listed in §12 MUST NOT appear outside a retired-callout.
6. **Cross-refs present.** The four requirements of §13 are satisfied with working relative links.
7. **Code snippets look syntactically plausible.** Rust fences parse as Rust (balanced braces, `impl ... for ...` blocks complete, `use` paths resolve against current crates). The reviewer does not compile them but performs a visual scan.

A pattern passes only when all seven checks pass.

## File-naming conventions

- Flow patterns at top level: `pattern_<flow_snake>.md`. Examples: `pattern_authorize.md`, `pattern_capture.md`, `pattern_refund.md`, `pattern_psync.md`, `pattern_void.md`, `pattern_rsync.md`, `pattern_setup_mandate.md`, `pattern_repeat_payment.md`, `pattern_incoming_webhook.md`.
- PM patterns under `authorize/`: `authorize/<pm_snake>/pattern_authorize_<pm_snake>.md`. Examples: `authorize/card/pattern_authorize_card.md`, `authorize/wallet/pattern_authorize_wallet.md`, `authorize/bank_debit/pattern_authorize_bank_debit.md`.
- Sub-patterns (qualified variants of a PM pattern): `authorize/<pm_snake>/pattern_authorize_<pm_snake>_<qualifier_snake>.md`. Examples: `authorize/card/pattern_authorize_card_ntid.md`, `authorize/card/pattern_authorize_card_3ds.md`.
- All filenames MUST be lowercase snake_case. No camelCase, no hyphens, no spaces.
- Directory names match their pattern's `<pm_snake>` exactly.

## Failure → revision loop

A FAIL verdict from the Wave-8 reviewer returns the artifact to its originating author agent with a structured list of failed rubric checks (§14). The author performs one revision cycle and resubmits. If the second submission also FAILs, the artifact escalates to human review rather than entering a third autonomous revision. The reviewer MUST cite rubric-check numbers (1-7) when failing; the author MUST address each cited check in the revision. Revision commits MUST preserve the file path; authors do not rename a pattern during revision. Escalated artifacts block their downstream waves until a human reviewer resolves them; the orchestrator is responsible for re-queueing the PR after human approval.

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
