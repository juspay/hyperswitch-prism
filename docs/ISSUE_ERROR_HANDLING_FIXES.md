# Issue: SDK Error Handling Fixes

## Summary

Fix inconsistencies in SDK error handling for RequestError, ResponseError, and NetworkError. Align with ErrorSwitch logic, remove side effects, and improve error semantics for developers.

---

## Problem Statement

1. **Wrong error conversion path:** ConnectorError is converted directly to RequestError/ResponseError via ReportInto, which always returns `status_code=400` and `error_code=None`. This ignores the correct per-variant mapping in ErrorSwitch (ucs_env).

2. **Side effect in ReportInto:** `eprintln!` runs on every error, polluting stderr.

3. **Unclear error source:** RequestError and ResponseError share the same structure; integrators cannot easily distinguish req_transformer vs res_transformer vs network failure.

4. **Missing tracing/debug support:** No opt-in for stack trace or error chain in sandbox, despite SDK being used by developers.

---

## Proposed Solution

### 1. Unify error conversion path

**Current (wrong):**
```
ConnectorError ──► ReportInto ──► RequestError (always 400, no error_code)
ApplicationErrorResponse ──► ReportInto ──► RequestError
```

**Proposed:**
```
ConnectorError ──► ErrorSwitch ──► ApplicationErrorResponse ──► ReportInto ──► RequestError
ApplicationErrorResponse ────────────────────────────────────► ReportInto ──► RequestError
```

- Use existing `ErrorSwitch` in `ucs_env/error.rs` for ConnectorError → ApplicationErrorResponse.
- Remove `ReportInto` impl for ConnectorError in `domain_types/errors.rs`.
- In `backend/ffi/src/macros.rs`, convert ConnectorError via ErrorSwitch before ReportInto:

```rust
.map_err(|e| {
    let app_err = e.current_context().switch();
    let app_report = e.change_context(app_err);
    <Report<ApplicationErrorResponse> as ReportInto<RequestError>>::report_into(app_report)
})?;
```

### 2. Correct status_code and error_code per ConnectorError variant

| ConnectorError | status_code | error_code | Where |
|----------------|-------------|------------|-------|
| RequestTimeoutReceived | 504 | REQUEST_TIMEOUT | res_transformer |
| FailedToObtainCertificate | 500 | FAILED_TO_OBTAIN_CERTIFICATE | req_transformer |
| FailedToObtainCertificateKey | 500 | FAILED_TO_OBTAIN_CERTIFICATE_KEY | req_transformer |
| ResponseDeserializationFailed | 502 | RESPONSE_DESERIALIZATION_FAILED | res_transformer |
| UnexpectedResponseError | 502 | UNEXPECTED_RESPONSE | res_transformer |
| ProcessingStepFailed | 500 | PROCESSING_STEP_FAILED | res_transformer |
| MissingRequiredField | 400 | MISSING_REQUIRED_FIELD | req or res |
| InvalidWalletToken | 400 | INVALID_WALLET_TOKEN | req or res |
| NotSupported | 400 | NOT_SUPPORTED | req or res |
| NotImplemented | 501 | NOT_IMPLEMENTED | req or res |

**Note:** ErrorSwitch already has this mapping. Use it; do not duplicate in ReportInto.

### 3. Add source field to proto

Add `source` to RequestError and ResponseError:

- `req_transformer` — request build failed
- `res_transformer` — response parse failed
- `network` — HTTP layer (NetworkError; separate type)

Enables integrators to distinguish error origin.

### 4. Remove eprintln! from ReportInto

**Location:** `backend/domain_types/src/errors.rs`

**Current:**
```rust
fn report_into(self) -> T {
    let ctx = self.current_context();
    eprintln!("Error: {:?}", ctx);  // ← Remove
    ...
}
```

**Fix:** Remove the `eprintln!` line. Use `tracing::error!` if logging is needed; do not print to stderr by default.

### 5. Do NOT expose error chain or stack trace

**Decision:** Do not add error_chain or stack_entries to the proto by default.

**Reasoning:**
- Payment industry standard (Stripe, Braintree): structured errors only
- Security: avoid leaking paths, env, internals
- Integrators need actionable fields (message, code, field), not raw chain
- Even in sandbox: better not to expose (per discussion update)

### 6. Optional: tracing in sandbox

**Out of scope for this issue.** If tracing/debug is needed in sandbox:
- Add `RequestConfig.debug` (or similar) as opt-in
- Only when explicitly enabled; not by default

---

## Files to Modify

| File | Change |
|------|--------|
| `backend/ffi/src/macros.rs` | Use ErrorSwitch for ConnectorError before ReportInto (lines 84, 187) |
| `backend/domain_types/src/errors.rs` | Remove ReportInto impl for ConnectorError; remove eprintln! from ReportInto |
| `backend/grpc-api-types/proto/sdk_config.proto` | Optional: add `source` field to RequestError, ResponseError |

---

## Acceptance Criteria

- [ ] ConnectorError flows through ErrorSwitch before ReportInto
- [ ] RequestTimeoutReceived returns status_code=504, error_code=REQUEST_TIMEOUT
- [ ] FailedToObtainCertificate returns status_code=500, error_code=FAILED_TO_OBTAIN_CERTIFICATE
- [ ] MissingRequiredField returns status_code=400, error_code=MISSING_REQUIRED_FIELD
- [ ] eprintln! removed from ReportInto
- [ ] No error chain or stack trace in proto
- [ ] SDK tests pass for error flows

---

## References

- [ERROR_PATHS_AUDIT.md](./ERROR_PATHS_AUDIT.md) — full error path audit
- [ERROR_PROTO_SPEC.md](./ERROR_PROTO_SPEC.md) — proto structure spec
- `backend/ucs_env/src/error.rs` — ErrorSwitch impl for ConnectorError
