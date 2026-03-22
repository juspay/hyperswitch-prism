# Full Error Paths Audit

A comprehensive deep dive into all locations where errors are thrown in the connector-service SDK, categorized by type: **RequestError**, **ResponseError**, and **success response with `error` field**.

---

## 1. REQUEST ERROR (req_transformer failures)

**Proto:** `RequestError` (status, error_message, error_code, status_code)

**Flow:** `req_transformer` → `req_handler` → FFI returns `Err(RequestError)` → SDK raises `RequestError` exception

### 1.1 Config loading failure

| Location | Error | Trigger |
|----------|-------|---------|
| `ffi/handlers/payments.rs:17-28` | `RequestError` (direct) | `load_config()` fails (invalid TOML, ConnectorError::GenericError) |

### 1.2 ApplicationErrorResponse → RequestError (via ReportInto)

| Location | Source | Trigger |
|----------|--------|---------|
| `ffi/macros.rs:59` | `connectors_with_connector_config_overrides()` | Config override parsing fails (CANNOT_CONVERT_TO_JSON, INVALID_MITM_CA_CERT, etc.) |
| `ffi/macros.rs:67` | `ForeignTryFrom` for `$resource_common_data_type` | Flow data construction fails (e.g. PaymentFlowData from payload + connectors + metadata) |
| `ffi/macros.rs:71` | `ForeignTryFrom` for `$request_data_type` | Request data validation fails (e.g. PaymentsAuthorizeData from payload) |

**ApplicationErrorResponse variants** (from `domain_types/errors.rs`):
- `MissingRequiredField { field_name }` (IR_04)
- `InvalidDataFormat`, `InvalidDataValue`, `InvalidRequestData` (IR_05, IR_06, IR_07)
- `InvalidConnectorConfiguration`, `CurrencyNotSupported`, etc.
- `InvalidConnector`, `InvalidConnectorConfig` (from ucs_interface_common)
- Many more (IR_01–IR_40+)

### 1.3 ConnectorError → RequestError (via ReportInto)

| Location | Source | Trigger |
|----------|--------|---------|
| `ffi/macros.rs:84` | `connector_integration.build_request_v2()` | Connector's req_transformer fails |

**ConnectorError variants** (from `domain_types/errors.rs`):
- `MissingRequiredField { field_name }` — developer didn't pass required field
- `MissingRequiredFields { field_names }` — multiple missing fields
- `InvalidDataFormat { field_name }` — wrong format
- `InvalidWalletToken { wallet_name }` — bad wallet token
- `NotImplemented(String)` — flow not supported for connector
- `NotSupported { message, connector }` — operation not supported
- `FlowNotSupported { flow, connector }` — flow not supported
- `MismatchedPaymentData` — payment method/data mismatch
- `UnexpectedResponseError` — (rare in req path)

---

## 2. RESPONSE ERROR (res_transformer failures)

**Proto:** `ResponseError` (status, error_message, error_code, status_code)

**Flow:** `res_transformer` → `res_handler` → FFI returns `Err(ResponseError)` → SDK raises `ResponseError` exception

### 2.1 Config loading failure

| Location | Error | Trigger |
|----------|-------|---------|
| `ffi/handlers/payments.rs:39-49` | `ResponseError` (direct) | `load_config()` fails |

### 2.2 ApplicationErrorResponse → ResponseError (via ReportInto)

| Location | Source | Trigger |
|----------|--------|---------|
| `ffi/macros.rs:149` | `connectors_with_connector_config_overrides()` | Same as req path |
| `ffi/macros.rs:157` | `ForeignTryFrom` for `$resource_common_data_type` | Flow data construction |
| `ffi/macros.rs:161` | `ForeignTryFrom` for `$request_data_type` | Request data validation |

### 2.3 ConnectorError → ResponseError (via ReportInto)

| Location | Source | Trigger |
|----------|--------|---------|
| `ffi/macros.rs:187` | `handle_connector_response()` | Connector's res_transformer fails |

**ConnectorError variants** (from connector transformers):
- `UnexpectedResponseError(bytes::Bytes)` — connector returned unexpected format

  **Connectors:** stripe, truelayer, fiuu, worldpayvantiv, aci, authorizedotnet, etc.
  
- `ResponseDeserializationFailed` — failed to parse connector response
- `ParsingFailed` — parsing error
- `MissingRequiredField { field_name }` — missing field in response parsing
- `InvalidDataFormat { field_name }` — wrong format in response
- `InvalidWalletToken { wallet_name }` — bad wallet token in response
- `NotImplemented`, `NotSupported`, `FlowNotSupported` — unsupported

### 2.4 handle_connector_response failure paths

**Input:** `classified_response` = `Ok(body)` for 2xx/3xx, `Err(body)` for 4xx/5xx

**When ConnectorError is returned:**
- `Err(body)` branch: `connector.get_error_response_v2(body)` or `get_5xx_error_response(body)` returns `Err(ConnectorError)` — e.g. cannot parse error body
- `Ok(body)` branch: `connector.handle_response_v2()` returns `Err(ConnectorError)` — e.g. cannot parse success body

### 2.5 ApplicationErrorResponse → ResponseError (via ReportInto)

| Location | Source | Trigger |
|----------|--------|---------|
| `ffi/macros.rs:190` | `generate_*_response()` | `ForeignTryFrom` fails when building final proto from RouterData |

---

## 3. SUCCESS RESPONSE WITH ERROR FIELD

**Proto:** `PaymentServiceAuthorizeResponse` (and similar) with `error: Some(ErrorInfo { connector_details, issuer_details })`

**Flow:** Connector returns 4xx/5xx → `handle_connector_response` parses error body → `RouterData { response: Err(ErrorResponse) }` → `generate_*_response` produces success proto with `error` populated

### 3.1 handle_connector_response success path (4xx/5xx)

```text
response = Err(body)  // 4xx/5xx HTTP body
  → connector.get_error_response_v2(body) or get_5xx_error_response(body)
  → returns Ok(ErrorResponse { code, message, reason, status_code, ... })
  → updated_router_data.response = Err(error)
  → returns Ok(router_data)
```

### 3.2 Connector transformers returning Err(ErrorResponse)

Connectors return `response: Err(ErrorResponse)` when they **successfully parse** the connector's error body:

| Connector | Location | When |
|-----------|----------|------|
| **Stripe** | `transformers.rs` | `handle_response` maps Stripe error JSON → `Err(ErrorResponse)` |
| **Zift** | `transformers.rs` | `response_code` indicates failure → `Err(ErrorResponse)` |
| **Xendit** | `transformers.rs` | `status == Failure` → `Err(ErrorResponse)` |
| **WorldpayXML** | `transformers.rs` | `response.reply.error` or `order_status.error` → `Err(ErrorResponse)` |
| **Authorize.net** | `transformers.rs` | `TransactionResponseError` variant → `Err(ErrorResponse)` |
| **ACI** | `transformers.rs` | Status indicates failure → `Err(ErrorResponse)` |
| **FIUU** | `transformers.rs` | Error response parsed → `Err(ErrorResponse)` |
| **Tsys** | `transformers.rs` | `get_error_response()` for error responses |
| **Trustpay** | `transformers.rs` | `result_info.result_code` indicates failure |
| **Paybox** | `transformers.rs` | `response_code` indicates failure |
| **Iatapay** | `transformers.rs` | `failure_code` in response |
| **Cybersource** | `transformers.rs` | Error details in response |
| **Celero** | `transformers.rs` | Error details, MISSING_DATA, etc. |
| **Bamboraapac** | `transformers.rs` | Response indicates failure |
| **Barclaycard** | `transformers.rs` | Error response variant |
| **Adyen** | `transformers.rs` | `result.error_code` |

### 3.3 generate_*_response functions setting error field

All these functions have an `Err(e)` branch that sets `error: Some(ErrorInfo { ... })`:

| Function | File:Line | Error branch |
|----------|-----------|--------------|
| `generate_payment_authorize_response` | types.rs:4005 | `Err(ErrorResponse)` → connector_details, issuer_details |
| `generate_payment_capture_response` | types.rs:6938 | `Err(e)` |
| `generate_payment_void_response` | types.rs:4711 | `Err(e)` |
| `generate_payment_void_post_capture_response` | types.rs:4816 | `Err(e)` |
| `generate_payment_incremental_authorization_response` | types.rs:6807 | `Err(e)` |
| `generate_refund_response` | types.rs:5722 | `Err(e)` |
| `generate_refund_sync_response` | types.rs:5800, 6108, 6449 | Multiple |
| `generate_payment_sync_response` | types.rs:3845 | `Err(err)` |
| `generate_setup_mandate_response` | types.rs:7926 | `Err(err)` |
| `generate_defend_dispute_response` | types.rs:8044 | `Err(e)` |
| `generate_accept_dispute_response` | types.rs:5467 | `Err(e)` |
| `generate_submit_evidence_response` | types.rs:5590 | `Err(e)` |
| `generate_session_token_response` | types.rs:8086 | `Err(e)` |
| `generate_create_payment_method_token_response` | types.rs:8927 | `Err(e)` |
| `generate_create_connector_customer_response` | types.rs:9072 | `Err(e)` |
| `generate_repeat_payment_response` | types.rs:9398 | `Err(err)` |
| `generate_create_order_response` | types.rs:3799 | `Err(err)` |
| `generate_access_token_response_data` | types.rs:4861 | (different structure) |

---

## 4. SPECIAL CASE: handle_event_transformer

**Flow:** Single-step webhook processing (no HTTP round-trip)

| Location | Error Type | Trigger |
|----------|------------|---------|
| `ffi/handlers/payments.rs:117-124` | `FfiPaymentError` (not RequestError/ResponseError) | Config load fails |
| `services/payments.rs:handle_event_transformer` | `FfiPaymentError` | Webhook parsing, signature verification, etc. |

**Note:** `handle_event` uses `FfiPaymentError` → `UniffiError::HandlerError`, not proto RequestError/ResponseError.

---

## 5. ERROR CONVERSION (ReportInto)

**Source types:** `ConnectorError`, `ApplicationErrorResponse`

**Target:** `RequestError`, `ResponseError` (both get same conversion)

```rust
// domain_types/errors.rs:1115-1125
impl_report_into!(ConnectorError, |e: &ConnectorError| {
    (Some(e.to_string()), None, Some(400))
});
impl_report_into!(ApplicationErrorResponse, |e: &ApplicationErrorResponse| {
    let api_error = e.get_api_error();
    (Some(api_error.error_message), Some(api_error.sub_code), Some(api_error.error_identifier))
});
```

**Result:** `RequestError`/`ResponseError` with status=Pending, error_message, error_code (from ApiError.sub_code), status_code.

---

## 6. NETWORK ERROR (SDK HTTP layer)

**Proto:** `NetworkError` (NetworkErrorCode, message, status_code)

**Flow:** SDK's HTTP client (Python httpx, Rust reqwest, etc.) — **does NOT cross FFI**. Thrown before/after the HTTP call.

| Code | When |
|------|------|
| CONNECT_TIMEOUT_EXCEEDED | Connection timeout |
| RESPONSE_TIMEOUT_EXCEEDED | Read timeout |
| TOTAL_TIMEOUT_EXCEEDED | Total request timeout |
| NETWORK_FAILURE | DNS, connection refused, TLS, etc. |
| URL_PARSING_FAILED | Bad URL from req_transformer |
| RESPONSE_DECODING_FAILED | Cannot read response body |
| INVALID_CA_CERT | Bad CA cert in config |
| INVALID_PROXY_CONFIGURATION | Bad proxy config |
| CLIENT_INITIALIZATION_FAILURE | HTTP client init failed |

---

## 7. SUMMARY BY CATEGORY

### 7.1 RequestError (developer mistake / config / SDK)

| Category | Examples |
|----------|----------|
| Missing required field | `billing_address.email`, `payment_method_data` |
| Invalid format | `InvalidDataFormat`, `InvalidWalletToken` |
| Config | `connectors_with_connector_config_overrides` fails |
| Connector limitation | `NotImplemented`, `NotSupported`, `FlowNotSupported` |
| Request build failure | `build_request_v2` returns ConnectorError |

### 7.2 ResponseError (connector response parsing / SDK)

| Category | Examples |
|----------|----------|
| Unparseable response | `UnexpectedResponseError`, `ResponseDeserializationFailed` |
| Missing field in response | `MissingRequiredField` during res parsing |
| Config | Same as req |
| Generate response failure | `generate_*_response` ForeignTryFrom fails |

### 7.3 Success + error field (connector business error)

| Category | Examples |
|----------|----------|
| Card declined | 4xx/5xx with parseable body (Stripe, etc.) |
| Invalid request to connector | Connector returned error JSON |
| 5xx from connector | `get_5xx_error_response` parses body |

### 7.4 NetworkError (transport)

| Category | Examples |
|----------|----------|
| Timeout | Connect, response, total |
| Connection failure | DNS, refused, TLS |
| Config | Invalid URL, CA cert, proxy |

---

## 8. PROTO CATEGORIZATION RECOMMENDATIONS

Based on this audit:

| Error Type | Proto | Suggested additions |
|------------|-------|---------------------|
| **RequestError** | RequestError | `flow`, `reason` (enum), `field_violations`, `suggestion` |
| **ResponseError** | ResponseError | `flow`, `reason`, `field_violations`, `suggestion`, `stack_entries` (opt-in) |
| **Success + error** | ErrorInfo (inside response) | Already has connector_details, issuer_details — consider `retryable`, `doc_url` |
| **NetworkError** | NetworkError | `suggestion`, `config_hint` |

**Reason codes to consider:**
- RequestError: `MISSING_REQUIRED_FIELD`, `INVALID_FORMAT`, `INVALID_CONFIG`, `NOT_SUPPORTED`
- ResponseError: `UNEXPECTED_RESPONSE`, `PARSE_FAILED`, `MISSING_FIELD`, `NOT_SUPPORTED`

---

## 9. QUICK REFERENCE: Error Type by Trigger

| Trigger | Error Type | Proto |
|---------|------------|-------|
| Config load fails | RequestError / ResponseError | RequestError, ResponseError |
| connectors_with_connector_config_overrides fails | RequestError / ResponseError | ApplicationErrorResponse → ReportInto |
| ForeignTryFrom (flow_data, request_data) fails | RequestError / ResponseError | ApplicationErrorResponse → ReportInto |
| build_request_v2 fails | RequestError | ConnectorError → ReportInto |
| handle_connector_response fails (parse error) | ResponseError | ConnectorError → ReportInto |
| generate_*_response fails | ResponseError | ApplicationErrorResponse → ReportInto |
| Connector 4xx/5xx, parseable body | Success + error field | ErrorInfo in response |
| HTTP client timeout/failure | NetworkError | NetworkError (SDK-side) |
| handle_event_transformer fails | FfiPaymentError | UniffiError (not proto) |
