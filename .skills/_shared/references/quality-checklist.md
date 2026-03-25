# Quality Checklist Reference

Condensed quality rules for UCS connector implementations. Derived from the
GRACE quality review system, feedback database, and learnings.

---

## 1. Pre-Submission Checklist

- [ ] `cargo build` passes with zero errors in the connector crate
- [ ] All implemented flows (Authorize, PSync, Capture, Refund, RSync, Void) compile
- [ ] No warnings related to unused imports, dead code, or unused variables
- [ ] Every flow follows its corresponding pattern file (pattern_authorize.md, etc.)
- [ ] Macro definitions are complete in `create_all_prerequisites!` and `macro_connector_implementation!`

---

## 2. UCS Architecture Compliance

These are CRITICAL -- violations block approval (score -20 each).

- [ ] Use `ConnectorIntegrationV2`, never legacy `ConnectorIntegration`
- [ ] Use `RouterDataV2` throughout, never `RouterData`
- [ ] Import from `domain_types`, never from `hyperswitch_domain_models` directly
- [ ] Connector struct is generic: `ConnectorName<T: PaymentMethodDataTypes>`
- [ ] All trait bounds properly defined with `Debug + Sync + Send + 'static + Serialize`
- [ ] Payment flows use `PaymentFlowData`, refund flows use `RefundFlowData`

---

## 3. Status Mapping Rules

- [ ] Status is ALWAYS derived from the connector response -- never hardcoded
- [ ] A dedicated status enum exists for connector-specific statuses (deserialized from response)
- [ ] Use enum matching, not string comparison (`match response.status` not `match response.status.as_str()`)
- [ ] All known connector status variants are mapped to the correct `AttemptStatus` / `RefundStatus`
- [ ] No catch-all `_ => AttemptStatus::Pending` that silently swallows unknown statuses
- [ ] Status mapping is consistent across related flows (Authorize/PSync share payment statuses, Refund/RSync share refund statuses)

**Wrong:**
```rust
// Hardcoded status -- NEVER do this
AttemptStatus::Charged
```

**Wrong:**
```rust
// String matching -- fragile and error-prone
match response.status.as_str() {
    "success" => AttemptStatus::Charged,
    _ => AttemptStatus::Pending,
}
```

**Correct:**
```rust
match response.status {
    ConnectorStatus::Success | ConnectorStatus::Completed => AttemptStatus::Charged,
    ConnectorStatus::Pending | ConnectorStatus::Processing => AttemptStatus::Pending,
    ConnectorStatus::Failed | ConnectorStatus::Declined => AttemptStatus::Failure,
}
```

---

## 4. Error Handling

- [ ] Use specific `ConnectorError` types (NotSupported, InvalidData, etc.)
- [ ] NotSupported errors include the exact feature/method name: `"Apple Pay is not supported"`
- [ ] No generic error messages -- all errors are descriptive
- [ ] Error response struct is defined and deserialized from connector error responses
- [ ] No `unwrap()` in production code -- propagate errors with `?`
- [ ] `change_context()` used to convert errors with added context

---

## 5. Naming Conventions

- [ ] Request/Response types: `{ConnectorName}{FlowName}{Request|Response}` (e.g., `StripeAuthorizeRequest`)
- [ ] Status enums: `{ConnectorName}{Context}Status` (e.g., `StripePaymentStatus`)
- [ ] Error types: `{ConnectorName}ErrorResponse`
- [ ] Module file is `{connector_name}.rs` (snake_case)
- [ ] Transformers file is `{connector_name}/transformers.rs`
- [ ] All struct and enum names use PascalCase
- [ ] All field names use snake_case matching the connector API's JSON keys via serde

---

## 6. Amount Handling

- [ ] Use the framework amount conversion utilities from `common_utils::types` (MinorUnit, StringMinorUnit)
- [ ] Currency unit (Base vs Minor) is configured correctly in `ConnectorCommon` or `create_all_prerequisites!`
- [ ] Amount converter is set up in `create_all_prerequisites!` macro
- [ ] Never implement custom currency conversion -- use `utils::to_currency_base_unit` and similar
- [ ] Verify zero-decimal currencies are handled correctly by the framework config

---

## 7. Authentication Pattern

- [ ] Auth type struct matches the connector's requirements (HeaderKey, BodyKey, SignatureKey, etc.)
- [ ] `build_headers` constructs auth headers from the auth type correctly
- [ ] API keys and secrets are sourced from `ConnectorAuthType` -- never hardcoded
- [ ] No credentials appear in error messages or logs
- [ ] All flows use the same authentication pattern consistently

---

## 8. Unused Code / Field Removal

- [ ] No fields hardcoded to `None` -- if always None, remove the field entirely
- [ ] No `Option` wrapper unless the field is truly optional per the connector API spec
- [ ] Only struct fields actually sent to / received from the connector API are present
- [ ] No dead code, unused imports, or commented-out blocks
- [ ] No defensive "just in case" fields -- keep structs minimal and clean
- [ ] Remove any scaffolding or placeholder code from `add_connector.sh`

---

## 9. Common Mistakes to Avoid

These are the most frequently observed issues from quality reviews:

| Mistake | Fix |
|---------|-----|
| Using `RouterData` instead of `RouterDataV2` | Replace with V2 types everywhere |
| Importing from `hyperswitch_domain_models` | Import from `domain_types` instead |
| Hardcoded status values | Derive status from connector response |
| String-based status matching | Define a status enum and deserialize into it |
| Fields always set to `None` | Delete the field from the struct |
| Generic catch-all error messages | Use specific error types with descriptive messages |
| Custom currency conversion logic | Use framework utilities from `common_utils` |
| Missing `<T>` generic on connector struct | Add `<T: PaymentMethodDataTypes>` |
| Using `ConnectorIntegration` trait | Use `ConnectorIntegrationV2` |
| Unnecessary `.clone()` calls | Borrow where possible, only clone when needed |
| `unwrap()` in production paths | Use `?` operator or explicit error handling |
| Reusing existing enums incorrectly (Currency, Country) | Reference `common_enums` for standard enums, don't redefine |

---

## 10. Macro Implementation Checks

- [ ] All flows defined in `create_all_prerequisites!` macro
- [ ] All flows use `macro_connector_implementation!` -- no manual trait impls
- [ ] HTTP methods match the connector API documentation (GET, POST, PUT, DELETE)
- [ ] Content types are correct (Json, FormData, FormUrlEncoded, or omitted)
- [ ] GET endpoints omit `curl_request` parameter; POST/PUT endpoints include it
- [ ] `member_functions` includes `build_headers` and `connector_base_url`
- [ ] Amount converter configured when the flow handles monetary amounts

---

## 11. Cross-Flow Consistency

- [ ] All flows use the same authentication pattern
- [ ] Shared types (status enums, error structs) are defined once and reused
- [ ] Similar operations are implemented similarly across flows
- [ ] Transformer logic is reused where applicable (shared helper functions)
- [ ] Naming style is uniform across all flow files

---

## 12. Final Verification Steps

Run these checks before declaring the connector complete:

1. **Build**: `cargo build` -- must pass cleanly
2. **Architecture**: Grep for `RouterData<` (not V2), `ConnectorIntegration<` (not V2), `hyperswitch_domain_models` -- all must return zero results in your connector files
3. **Hardcoded status**: Search for direct `AttemptStatus::Charged`, `AttemptStatus::Failure` etc. outside of a match arm mapping from connector response -- must be zero
4. **Dead fields**: Check every `None` assignment in request builders -- verify the field is conditionally used, not always None
5. **Error quality**: Verify every `NotSupported` error includes the specific unsupported item name
6. **Completeness**: Confirm all six core flows are implemented (Authorize, PSync, Capture, Refund, RSync, Void) plus any pre-auth flows required by the connector
7. **Status coverage**: Verify every status value documented in the connector's API spec has a mapping
8. **Struct cleanliness**: No unused fields, no unnecessary Option wrappers, no placeholder values
