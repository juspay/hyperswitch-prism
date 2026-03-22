# Error Proto Structure — World-Standard Specification

A revised error proto design with concrete reasoning based on Google API Design (AIP-193), Stripe, and payment industry standards.

---

## 1. Reference Standards

| Standard | Source | Key patterns |
|----------|--------|--------------|
| **Google AIP-193** | [cloud.google.com/apis/design/errors](https://cloud.google.com/apis/design/errors) | ErrorInfo (reason, domain, metadata), BadRequest (FieldViolation), Help (links), RetryInfo |
| **Google error_details.proto** | googleapis/google/rpc/error_details.proto | FieldViolation (field, description, reason), Help.Link (description, url) |
| **Stripe API** | stripe.com/docs/api/errors | code, message, param (field path), type |
| **gRPC Status** | grpc.io | code, message, details (repeated Any) |
| **Payment industry** | Stripe, Braintree, Adyen | Structured only; no stack traces; machine-readable codes |

---

## 2. Proposed Proto Structure

### 2.1 FieldViolation (reusable)

**Reference:** Google `BadRequest.FieldViolation`, Stripe `param`

```protobuf
// Field-level violation for machine-readable error handling.
// Aligns with Google BadRequest.FieldViolation and Stripe's param field.
message FieldViolation {
  // Dot-separated path to the field (e.g. "billing_address.email", "payment_method_data.wallet_token").
  // Enables programmatic highlighting and LLM-driven fixes.
  string field = 1;
  // Human-readable description of why the field failed.
  string description = 2;
  // Machine-readable reason (UPPER_SNAKE_CASE, max 63 chars). E.g. MISSING_REQUIRED_FIELD, INVALID_FORMAT.
  // Enables pattern matching without string parsing.
  optional string reason = 3;
  // Expected format/value hint. E.g. "Valid email string", "ISO 4217 currency code".
  optional string expected = 4;
  // Actual value if safe to expose (e.g. for validation errors). Omit for security-sensitive fields.
  optional string actual = 5;
}
```

**Reasoning:**
- **field:** Stripe uses `param`; Google uses `field`. Dot-separated path is standard.
- **description:** Human-readable; required for display.
- **reason:** Google uses UPPER_SNAKE_CASE for machine handling; enables `if (reason == "MISSING_REQUIRED_FIELD")`.
- **expected/actual:** Common in validation errors; helps LLMs suggest fixes.

---

### 2.2 ErrorSuggestion (reusable)

**Reference:** Google `Help`, Stripe docs links

```protobuf
// Actionable guidance for resolving the error.
// Aligns with Google Help and Stripe's documentation links.
message ErrorSuggestion {
  // Human-readable fix suggestion. E.g. "Add billing_address.email to your AuthorizeRequest".
  optional string suggestion = 1;
  // URL to documentation or troubleshooting. E.g. "https://docs.connector-service.io/errors/MISSING_REQUIRED_FIELD".
  optional string doc_url = 2;
  // Whether the client can retry with different input (true) or same request (false for config errors).
  // Enables automated retry logic.
  optional bool retryable = 3;
}
```

**Reasoning:**
- **suggestion:** Actionable; supports LLM-driven fixes.
- **doc_url:** Google Help.Link pattern; Stripe links to docs.
- **retryable:** Google RetryInfo; helps clients decide retry behavior.

---

### 2.3 RequestError (SDK FFI — req_transformer)

**Reference:** Google ErrorInfo + BadRequest, Stripe error structure

```protobuf
// Error returned by req_transformer FFI calls.
// Used when building the connector HTTP request fails.
message RequestError {
  // Payment status (e.g. Pending, Failure). Kept for backward compatibility.
  PaymentStatus status = 1;
  // Human-readable error message. Required for display and logging.
  optional string error_message = 2;
  // Machine-readable error code. E.g. IR_04, MISSING_REQUIRED_FIELD, REQUEST_TIMEOUT.
  // Enables programmatic handling; aligns with Stripe "code" and Google "reason".
  optional string error_code = 3;
  // HTTP-style status code (400, 401, 500, 504). Enables retry logic and monitoring.
  optional uint32 status_code = 4;

  // --- World-standard extensions ---

  // Flow where error occurred (e.g. "authorize", "capture"). Enables flow-specific handling.
  optional string flow = 5;
  // Machine-readable reason (UPPER_SNAKE_CASE). Canonical cause; use for pattern matching.
  optional string reason = 6;
  // Field-level violations. Aligns with Google BadRequest.field_violations.
  repeated FieldViolation field_violations = 7;
  // Actionable guidance. Aligns with Google Help.
  optional ErrorSuggestion suggestion = 8;
  // Identifies error source for debugging. "req_transformer" vs "res_transformer".
  optional string source = 9;
}
```

**Reasoning:**
- **status, error_message, error_code, status_code:** Existing; keep for compatibility.
- **flow:** Identifies which operation failed; useful for logging and monitoring.
- **reason:** Google ErrorInfo.reason; Stripe error.type; single canonical cause.
- **field_violations:** Google BadRequest; Stripe param; enables structured field-level errors.
- **suggestion:** Google Help; actionable for integrators and LLMs.
- **source:** Distinguishes req vs res errors when both use similar structure.

---

### 2.4 ResponseError (SDK FFI — res_transformer)

**Reference:** Same as RequestError; response-side failures

```protobuf
// Error returned by res_transformer FFI calls.
// Used when parsing the connector HTTP response fails.
message ResponseError {
  PaymentStatus status = 1;
  optional string error_message = 2;
  optional string error_code = 3;
  optional uint32 status_code = 4;

  // --- World-standard extensions ---

  optional string flow = 5;
  optional string reason = 6;
  repeated FieldViolation field_violations = 7;
  optional ErrorSuggestion suggestion = 8;
  optional string source = 9;
  // Snippet of raw response that caused parse failure (truncated, for debugging). Optional; omit if sensitive.
  optional bytes raw_response_snippet = 10;
}
```

**Reasoning:**
- Same structure as RequestError for consistency.
- **raw_response_snippet:** Optional; helps debug parse failures. Truncate and omit for sensitive data.

---

### 2.5 NetworkError (SDK HTTP layer)

**Reference:** Google ErrorInfo, transport-level errors

```protobuf
message NetworkError {
  NetworkErrorCode code = 1;
  optional string message = 2;
  optional uint32 status_code = 3;

  // --- World-standard extensions ---

  // Actionable guidance. E.g. "Ensure connector base_url is a valid HTTPS URL".
  optional ErrorSuggestion suggestion = 4;
  // Config hint. E.g. "ConnectorConfig.connector_config.stripe.base_url".
  optional string config_hint = 5;
}
```

**Reasoning:**
- **suggestion:** Transport errors often need config fixes; actionable guidance.
- **config_hint:** Points to the config field to fix; aligns with Google metadata.

---

### 2.6 ErrorInfo (success response — connector business error)

**Reference:** Existing structure; Google ErrorInfo, Stripe error object

```protobuf
message ErrorInfo {
  optional UnifiedErrorDetails unified_details = 1;
  optional IssuerErrorDetails issuer_details = 2;
  optional ConnectorErrorDetails connector_details = 3;

  // --- World-standard extensions ---

  // Whether retrying the same request may succeed (e.g. transient vs permanent decline).
  optional bool retryable = 4;
  // URL to documentation for this error type.
  optional string doc_url = 5;
}
```

**Reasoning:**
- **retryable:** Google RetryInfo; helps clients decide retry.
- **doc_url:** Google Help; Stripe links to docs.

---

## 3. Reason Codes (canonical enum)

**Reference:** Google UPPER_SNAKE_CASE, Stripe error codes

```protobuf
// Canonical error reasons for RequestError and ResponseError.
// Use for programmatic handling; add new codes as needed.
enum SdkErrorReason {
  SDK_ERROR_REASON_UNSPECIFIED = 0;
  // Developer mistake — fix input
  MISSING_REQUIRED_FIELD = 1;
  MISSING_REQUIRED_FIELDS = 2;
  INVALID_FORMAT = 3;
  INVALID_WALLET_TOKEN = 4;
  NOT_SUPPORTED = 5;
  NOT_IMPLEMENTED = 6;
  MISMATCHED_PAYMENT_DATA = 7;
  INVALID_CONFIG = 8;
  // SDK/connector internal
  REQUEST_TIMEOUT = 9;
  RESPONSE_DESERIALIZATION_FAILED = 10;
  UNEXPECTED_RESPONSE = 11;
  CONFIG_LOAD_FAILED = 12;
  INTERNAL_SERVER_ERROR = 13;
}
```

**Reasoning:** Stable enum for pattern matching; extend as needed.

---

## 4. What We Explicitly Do NOT Include

| Omitted | Reason (world standard) |
|---------|--------------------------|
| **Stack trace** | Stripe, Google, payment APIs do not expose. Security; not actionable for integrators. |
| **Error chain** | Implementation detail. Use structured fields instead. |
| **File paths** | Leak deployment info. Never in production errors. |
| **DebugInfo by default** | Google has it but for server-side. SDK: opt-in only. |

---

## 5. Summary: Alignment with Standards

| Field | Google | Stripe | Our design |
|-------|--------|--------|------------|
| reason/code | ErrorInfo.reason | error.code | reason, error_code |
| field path | BadRequest.field | param | field_violations[].field |
| description | FieldViolation.description | message | field_violations[].description |
| doc link | Help.Link | docs | suggestion.doc_url |
| retryable | RetryInfo | — | suggestion.retryable |
| status code | gRPC Code | — | status_code |
| flow/operation | — | — | flow (our addition) |

---

## 6. Migration Path

1. **Phase 1:** Add new optional fields (flow, reason, field_violations, suggestion). Backward compatible.
2. **Phase 2:** Populate from ErrorSwitch + ReportInto; fix ConnectorError path.
3. **Phase 3:** Document; encourage integrators to use structured fields.
4. **Phase 4:** Deprecate or simplify legacy string-only handling if needed.
