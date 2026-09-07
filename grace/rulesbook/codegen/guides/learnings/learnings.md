# UCS Connector Implementation Learnings & User Feedback

This file captures lessons learned from UCS connector implementations and user feedback to continuously improve AI-generated code quality.

## 📚 Implementation Learnings

### Key Patterns That Work Well

- **Always use the `create_all_prerequisites!` + `macro_connector_implementation!` pair.** Hand-written `ConnectorIntegrationV2` impls only make sense for flows with **no outbound call** (e.g. Kount's `PreAuthenticate`) — and even then, only override `get_call_connector_action` and friends. Everything else should go through `macro_connector_implementation!`.
- **Use `macro_connector_flow_status_impls!` to declare what is *not* supported.** Don't leave flows unimplemented silently — list them under `not_implemented: [...]` or `not_supported: [...]` so the trait is satisfied explicitly (see `twoc_twop_paco.rs`, `kount.rs`, `qwikcilver.rs`).
- **Base-URL helpers in `member_functions`.** Put `connector_base_url_payments` / `connector_base_url_refunds` / `connector_base_url_merchant_auth` in `create_all_prerequisites!` so every flow's `get_url` is a one-liner. The `merchant_auth` variant returns the same `base_url` for most connectors — Qwikcilver shows the pattern with three helpers.
- **`preprocess_request: true` + `preprocess_response: true` for envelope-wrapped APIs.** If the connector wraps bodies in JOSE/JWS/JWE (2C2P PACO's `application/jose`), implement `preprocess_request_bytes` / `preprocess_response_bytes` in `create_all_prerequisites!` `member_functions`. JSON is still your logical schema — the macros run the bytes through your preprocessors.
- **OAuth/session-token bootstrap lives in `ServerAuthenticationToken` with `MerchantAuthenticationFlowData`.** Set `ValidationTrait::should_do_access_token -> true`. Subsequent flows read the token from `resource_common_data.access_token` (or `FrmFlowData.access_token` for FRM flows). Both Qwikcilver (session JWT) and Kount (OAuth bearer) follow this pattern.
- **Error mapping should be defensive about the error envelope.** 2C2P PACO's `build_error_response` detects whether the error body is JOSE-wrapped before trying to parse it as JSON — connectors that wrap *success* responses sometimes also wrap *error* responses.
- **Return the correct error type from each callback.** `preprocess_request_bytes` and `get_url`/`get_headers`/`build_error_response` during request-building return `errors::IntegrationError`; `preprocess_response_bytes` and `build_error_response` return `errors::ConnectorError`. Picking the wrong one causes a `change_context` dance that obscures the real error (see the difference in `twoc_twop_paco.rs`).

### Common Pitfalls to Avoid

- **Don't use `RouterData` / `ConnectorIntegration` / `hyperswitch_*` crates** — UCS uses `RouterDataV2`, `ConnectorIntegrationV2`, and `domain_types` exclusively.
- **Don't hardcode a status** in the response transformer (e.g. `AttemptStatus::Charged`). Always map from the connector's status enum via a `map_{connector}_status_to_attempt_status` function.
- **Don't add `Option<T>` fields "just in case".** If the API docs don't list it as optional, don't add it. Struct cleanliness is enforced by the Quality Guardian.
- **Don't use `git add -A` / broad `git add`.** Stage only the connector's own files (`crates/integrations/connector-integration/src/connectors/{connector}*`).
- **Don't run `cargo test` during codegen.** Validation is done by `cargo build` + `grpcurl` against the running `grpc-server`. Integration tests are a separate phase (the Test Suite Agent runs `test-prism`).
- **Don't invent new connector-struct generics.** The pattern is `pub struct {ConnectorName}<T>` with `T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize` — that's what the macros expect.

### UCS-Specific Best Practices

- **Auth headers come from `connector_config`, not hardcoded.** `ConnectorCommon::get_auth_header` reads the `ConnectorSpecificConfig` — never embed credentials in source. (This is also why the PR agent scrubs diffs for `x-api-key` values.)
- **Sensitive values are `Secret<String>` / `Maskable<String>`.** Use `.peek()` to read for building URLs/headers, `.expose()` only when you must cross a FFI/serialization boundary. Never log a `Secret`.
- **`secondary_base_url` exists** on `connectors.{name}` in config — use it when the auth host differs from the API host (e.g. Kount uses `login.kount.com` for OAuth and `api.kount.com` for Orders).
- **Use `urlencoding::encode` for path segments** that come from request data (e.g. Kount's `orderId`, Qwikcilver's `wallet_number`, 2C2P's `orderNo`).
- **FRM flows use `FrmFlowData`.** For a fraud provider (Kount), the trait-bound is `connector_types::FrmServiceTrait`, and you stub unimplemented FRM flows with `macros::frm_flow_not_implemented!`.
- **`RedirectForm::Script { script_data }`** is the variant to return when the connector expects raw JS to run in the shopper's browser (Kount DDC). Never return a fake `<form>` when a `<script>` is what the SDK contract actually requires (see `kount.rs` `build_ddc_script` — it escapes interpolated strings via a local `js_string_escape`).
- **Composite HTTP endpoint**: connectors like Qwikcilver that need a token bootstrap are reachable via the composite HTTP handler (`crates/grpc-server/grpc-server/src/http/handlers/composite/`) which pre-invokes the `ServerAuthenticationToken` flow before the main flow. If `state.access_token` is already present, the bootstrap is skipped.

---

## 🎯 User Feedback Log

### Template for Feedback Entries:
```
### [DATE] [CONNECTOR_NAME] - [FLOW_NAME] Implementation
**Feedback**: [Positive/Negative/Neutral]
**Rating**: [Good/Needs Improvement/Bad]
**Comments**: [User's specific comments]
**Implementation Details**: [What was implemented]
**Lessons**: [What this teaches us for future implementations]
```

### 2026-05 — twoc_twop_paco — 6 core flows + VoidPC
**Feedback**: Positive
**Rating**: Good
**Implementation Details**: First connector to need `preprocess_request: true` (JOSE sign+encrypt of the outgoing body) and `preprocess_response: true` (JWE decrypt+verify of the incoming body).
**Lessons**: JSON is the logical schema even when the wire format is JOSE. Use `preprocess_*` flags; don't try to make `curl_request` produce the JWE itself. Error responses may *also* be JOSE-wrapped — detect envelope before parsing.

### 2026-06 — kount — FRM flows + PreAuthenticate
**Feedback**: Positive
**Rating**: Good
**Implementation Details**: First FRM integration (PreRiskCheck, FrmPaymentOutcome, FrmRefundProcessed) and first local-only `PreAuthenticate` (DDC script via `RedirectForm::Script`). Uses `create_amount_converter_wrapper!(connector_name: Kount, amount_type: StringMinorUnit)` at module scope.
**Lessons**: Fraud providers don't fit the payment-state-machine; they use `FrmFlowData` and `frm_types::*` instead. Local-only flows override `get_call_connector_action() -> HandleResponseWithoutBuildRequest` and implement `ConnectorIntegrationV2` manually.

### 2026-06 — qwikcilver — gift-card / wallet-provisioning
**Feedback**: Positive
**Rating**: Good
**Implementation Details**: First wallet-provisioning integration — `ServerAuthenticationToken` for session JWT + `Authorize` (REDEEM) + `Refund` (CANCELREDEEM) + `Recharge` + `CreatePaymentMethod` + `GetPaymentMethod` + `PaymentMethodEligibility`. All calls after the bootstrap send the session JWT via a `Bearer` header built in `build_authenticated_headers`.
**Lessons**: Use `MerchantAuthenticationFlowData` for the bootstrap; subsequent flows read `resource_common_data.access_token`. Non-payment flows (Recharge/CreatePaymentMethod/GetPaymentMethod) are valid `create_all_prerequisites!` entries with their own request/response types.

---

## 📊 Feedback Analysis

### Positive Patterns (Reuse These)
- `macro_connector_implementation!` + `create_all_prerequisites!` for everything that calls an HTTP endpoint.
- `macro_connector_flow_status_impls!` to declare unimplemented/unsupported flows explicitly.
- `preprocess_request_bytes` / `preprocess_response_bytes` for envelope-wrapped transports.
- `MerchantAuthenticationFlowData` for OAuth/session bootstraps.
- `RedirectForm::Script` for DDC/script-only redirects.
- [Add more as patterns recur across connectors]

### Areas for Improvement (Avoid These)
- Manual `ConnectorIntegrationV2` impls for plain HTTP flows (only justified for no-call flows).
- Hardcoded `AttemptStatus` in response transformers.
- `Option<T>` on fields that the API docs mark as required.
- `cargo test` in codegen — GRACE's validation is `cargo build` + `grpcurl`.
- [Add implementation approaches that received negative feedback]

### User Preferences
- Specific `NotSupported` messages (e.g. "Apple Pay is not supported") instead of generic ones.
- Error enums for connector statuses (e.g. `{ConnectorName}Status`) — never `String` status matching.
- [Add preferences for error handling, structure, etc. as observed]

---

## 🔄 Learning Evolution

### Current Implementation Level
**Level**: Baseline (following UCS patterns)
**Focus Areas**:
- Flow independence
- Code reuse without duplication
- Proper UCS architecture compliance

### Learning Milestones
- [ ] **Milestone 1**: Collect initial feedback (5+ flows)
- [ ] **Milestone 2**: Identify user preferences (10+ flows)
- [ ] **Milestone 3**: Optimize based on feedback (20+ flows)
- [ ] **Milestone 4**: Highly refined implementations (50+ flows)

---

## 💡 Implementation Guidelines Based on Learning

### Code Structure Preferences
- Macros (`create_all_prerequisites!`, `macro_connector_implementation!`, `macro_connector_flow_status_impls!`) over manual trait impls — only break out when there's no HTTP call (see Kount `PreAuthenticate`).
- Module-scope `create_amount_converter_wrapper!` when the connector needs a non-default amount unit (see `kount.rs`, `qwikcilver.rs`).
- [Update based on user feedback]

### Error Handling Patterns
- Detect wrapped envelopes before parsing errors (see `twoc_twop_paco.rs` `build_error_response`).
- Distinct `IntegrationError` (request-building phase) vs `ConnectorError` (response-handling phase) for clearer diagnostics.
- [Update based on user feedback]

### Request/Response Transformation Approaches
- Keep request structs minimal — only fields actually used by the connector API.
- Status enums (`{ConnectorName}Status`) with a dedicated mapper, never stringly-typed.
- [Update based on user feedback]

### Testing and Validation Preferences
- `cargo build` for compile-time correctness, `grpcurl` against a live local `grpc-server` for runtime correctness. Never `cargo test` in codegen phase; hardening is the Test Suite Agent's job via `test-prism`.
- [Update based on user feedback]

---

## 🔧 Feedback Integration Process

1. **After Each Flow Implementation**: Ask for optional feedback
2. **Store Feedback**: Add to this file using the template above
3. **Analyze Patterns**: Look for recurring positive/negative feedback
4. **Update Guidelines**: Modify implementation approach based on learnings
5. **Apply Learning**: Use insights in future implementations

---

## 📈 Success Metrics

### Feedback Quality Indicators
- **Positive Feedback Rate**: [Track percentage of positive feedback]
- **Implementation Efficiency**: [Track time to implement flows]
- **User Satisfaction**: [Track overall satisfaction with generated code]
- **Learning Application**: [Track how well feedback is incorporated]

### Continuous Improvement Goals
- Increase positive feedback rate over time
- Reduce implementation issues reported by users
- Improve code quality consistency
- Build comprehensive knowledge base for UCS development

---

**Note**: All feedback is voluntary and helps improve the AI's ability to generate high-quality UCS connector code. Users can always skip feedback requests without any impact on the implementation process.
