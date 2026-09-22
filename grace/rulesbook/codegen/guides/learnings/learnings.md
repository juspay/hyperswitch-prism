# UCS Connector Implementation Learnings & User Feedback

This file captures lessons learned from UCS connector implementations and user feedback to continuously improve AI-generated code quality.

## 📚 Implementation Learnings

### Key Patterns That Work Well
- [Add successful patterns based on experience]
- [Note what consistently receives positive feedback]

### Common Pitfalls to Avoid

These are contract facts verified against HEAD, not preferences. Each one has been
seen in generated code and each one fails the build or a review.

- **`RouterDataV2` takes four type parameters** —
  `<Flow, ResourceCommonData, Request, Response>`. Three is E0107. `status` lives
  on `resource_common_data` (`PaymentFlowData` / `RefundFlowData`), not on
  `RouterDataV2`.
- **`SourceVerification` and `BodyDecoding` are non-generic.** One impl per
  connector. A per-flow `impl<T> SourceVerification<Flow, Data, Req, Resp>` is
  E0107. Exemplar: `connectors/travelhub.rs:175`.
- **`PaymentsCaptureData` has no `payment_amount`** (E0609) and its
  `amount_to_capture` is `i64`, not an `Option` (`.is_none()` is E0599). Partial
  capture is detected by comparing `amount_to_capture` against the authorized
  amount the connector itself returned.
- **Webhook argument counts are fixed.** `get_event_type` takes one argument
  besides `&self`; `process_payment_webhook` takes four. Three to either is E0061.
  The error type is `WebhookError`, not `IntegrationError`. There is no
  `transformation_status` field and no `WebhookTransformationStatus` type (E0560).
- **`ConnectorError` has exactly five variants**, four of them struct variants
  requiring `context`. `InvalidData`, `NotImplemented(..)` and `InvalidCard` do
  not exist on it (E0599) — those names belong to `IntegrationError`.
- **Auth comes from `req.connector_config: ConnectorSpecificConfig`.**
  `connector_auth_type` was deleted from `RouterDataV2` on 2026-03-14
  (`a7a696c3a`). `get_auth_header(&ConnectorAuthType)` no longer matches the
  trait (E0407).
- **`build_error_response` takes three parameters** besides `&self`:
  `(res, Option<&mut events::Event>, &ConnectorSpecificConfig)`. `ConnectorEvent`
  is not the type used here, and `set_error_response_body` is not a method on
  `events::Event`. The same third-parameter change applies to
  `get_error_response_v2` and `get_5xx_error_response`.
- **Enum struct variants have no functional-update syntax.** Every field of
  `PaymentsResponseData::TransactionResponse` (11) must be listed or it is E0063.
  `RefundsResponseData` has 4 fields.
- **`ErrorResponse::attempt_status` is `Option<FlowStatus>`**, not
  `Option<AttemptStatus>`. `ErrorResponse` has 13 fields and an
  `impl Default`, so prefer `..Default::default()`.

### UCS-Specific Best Practices

- **Never `unwrap_or_default()` an error code or message.** Use
  `common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE}` —
  `.unwrap_or_else(|| NO_ERROR_CODE.to_string())`. Real connectors do this 247
  times; empty strings tell the merchant nothing.
- **Terminal status on the shared error path must be flow-aware.** Hardcoding
  `Some(AttemptStatus::Failure)` reports a charged payment as FAILURE (and no
  longer typechecks). A blanket `attempt_status: None` is equally wrong — a
  hard-declined refund then stays Pending and keeps retrying. Exemplar:
  `connectors/flywire.rs:362-370`; minimal form `connectors/noon.rs:499-512`.
- **Status mapping needs both halves.** `#[serde(other)] Unknown` at the
  deserialization layer so an unknown wire value does not fail the parse, and an
  exhaustive `match` with no `_ =>` arm at the mapping layer so the compiler flags
  a newly added status. Reviewers demand both.
- **Read the vendor spec for the amount unit.** There are five unit types in
  `common_utils::types` (`MinorUnit`, `StringMinorUnit`, `StringMajorUnit`,
  `FloatMajorUnit`, `StringTwoDecimalUnit`). "Default to `StringMinorUnit` if
  unclear" is wrong about four times in five — the HEAD distribution is
  StringMajorUnit 34, FloatMajorUnit 26, MinorUnit 21, StringMinorUnit 19.
- **In-band 2xx failure must return `Err(ErrorResponse { .. })`**, branching on a
  success predicate — reference `utils::is_payment_failure` in
  `crates/types-traits/domain_types/src/utils.rs`.
- **Use the three stub macros** in
  `crates/integrations/connector-integration/src/connectors/macros.rs`:
  `macro_connector_flow_status_impls!` (`not_implemented:` / `not_supported:`;
  used by 112 of 112 connectors), `macro_connector_local_flow_implementation!`
  (flows with no outbound HTTP call), and
  `macro_connector_payout_implementation!` (payout stubs).
- **Verify before you copy.** Any type name, field, or signature taken from a
  guide should be checked with `rg`/`sed` against `crates/` first. The contract
  has moved several times; a stale snippet looks right and does not compile.

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

---

## 📊 Feedback Analysis

### Positive Patterns (Reuse These)
- [Patterns that consistently receive good feedback]
- [Code structures users appreciate]
- [Implementation approaches that work well]

### Areas for Improvement (Avoid These)
- [Patterns that received negative feedback]
- [Common issues users report]
- [Implementation approaches to avoid]

### User Preferences
- [What users consistently prefer in code style]
- [Specific feedback about UCS implementations]
- [Preferences for error handling, structure, etc.]

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
- [Update based on user feedback]

### Error Handling Patterns
- [Update based on user feedback]

### Request/Response Transformation Approaches
- [Update based on user feedback]

### Testing and Validation Preferences
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