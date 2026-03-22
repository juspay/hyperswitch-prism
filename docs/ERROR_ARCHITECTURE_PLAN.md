# Error Architecture Plan (Finalized)

## Core Error Types (4 + 1 Wrapper)

| Type | Phase | Responsibility |
|---|---|---|
| **ConnectorRequestError** | Inbound (A) + Mapping (B) | "Pre-flight" logic: validation, encoding, and mapping. |
| **ApiClientError** | Transport (C) | Infrastructure: HTTP client setup, timeouts, and network failures. |
| **ConnectorResponseError**| Parsing (D) + Egress (E) | "Post-flight" logic: decoding, parsing, and formatting. |
| **WebhookError** | Webhooks | Async notifications: signature verification and payload recovery. |
| **ConnectorFlowError** | Orchestration | **Wrapper** for the gRPC round-trip pipeline. |

---

## Decisions

### 1. No Chained Mapping
Each core type implements `IntoGrpcStatus` directly. We avoid `RequestError` -> `ContractError` chaining to preserve rich stack traces and original context.

### 2. ApiClientError remains specialized
We decided **not** to redistribute `ApiClientError` variants into Request/Response types. It remains the source of truth for the **Transport Layer**. If `call_connector_api` fails, it always returns `ApiClientError`.

### 3. Wrapper Name: `ConnectorFlowError`
The orchestration wrapper is named `ConnectorFlowError` to distinguish it from the legacy monolithic `ConnectorError` and to align with the "Flow" concept used in `PaymentFlowData`.

### 4. Fault Attribution via Status Codes
- `ConnectorRequestError` -> `INVALID_ARGUMENT` (400) or `FAILED_PRECONDITION` (422).
- `ApiClientError` -> `UNAVAILABLE` (503) or `DEADLINE_EXCEEDED` (408).
- `ConnectorResponseError` -> `INTERNAL` (500).

---

## Proto Contract (FFI/SDK)

To provide actionable guidance to SDK consumers, internal errors map to enriched Proto structures.

### IntegrationError (Request Side)
| Field | Source | Example |
|---|---|---|
| `error_message` | `self.to_string()` | "Missing required field 'amount'" |
| `error_code` | `self.error_code()` | `MISSING_REQUIRED_FIELD` |
| `suggested_action`| Phase-aware guidance | "Provide field 'amount' in your request" |
| `doc_url` | Metadata | Link to connector integration guides |

### ConnectorResponseTransformationError (Response Side)
| Field | Source | Example |
|---|---|---|
| `error_message` | "Failed to parse connector response" | "Expected JSON, got HTML" |
| `error_code` | `self.error_code()` | `RESPONSE_DESERIALIZATION_FAILED` |
| `http_status_code`| `error_stack` Attachment | `401`, `500`, etc. |

---

## Implementation Status

- [x] **Trait Updates:** `ConnectorIntegrationV2` uses `ConnectorRequestError` and `ConnectorResponseError`.
- [x] **Orchestrator:** `execute_connector_processing_step` returns `ConnectorFlowError`.
- [x] **FFI Macros:** `req_transformer` and `res_transformer` isolate failure phases.
- [x] **Domain Types:** `ForeignTryFrom` implementations updated to return `ConnectorRequestError`.
- [x] **Boundary:** `IntoGrpcStatus` implemented for all new types.
- [x] **Removal:** `ApplicationErrorResponse` removed from core logic.
