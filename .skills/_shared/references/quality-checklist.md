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

> This section is the *build* gate. It is not sufficient to open a PR. Section 15
> (**Pre-Flight Gate**) is the last thing you run before pushing, and section 16 lists what
> the PR body must disclose. Certification — which is merge-blocking — is a separate
> document: `.skills/_shared/references/certification.md`.

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
- [ ] Status mapping is consistent across related flows (Authorize/PSync share payment statuses, Refund/RSync share refund statuses)
- [ ] **Both halves of the unknown-status rule are present** (see below): `#[serde(other)] Unknown`
      at the deserialization layer, and an exhaustive `match` with no `_ =>` arm at the
      status-mapping layer

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
    ConnectorStatus::Unknown => AttemptStatus::Pending,
}
```

### Unknown statuses: handle them at the right layer

Reviewers ask for **both** halves, and rejecting one half is the single most common status review
comment. They are not interchangeable:

**Deserialization layer -- `#[serde(other)]` is required.** Without it, a status string the vendor
adds next quarter makes the whole response fail to deserialize, and a real charge surfaces as a
transport error. 77 enums across 33 connectors at HEAD carry this attribute:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ExamplePayStatus {
    Succeeded,
    Pending,
    Failed,
    #[serde(other)]
    Unknown,          // absorbs any value not listed above
}
```

**Status-mapping layer -- no catch-all `_ =>`.** Match every variant explicitly, `Unknown`
included. An exhaustive match means the compiler tells you when someone adds a variant; a `_ =>`
arm silently maps it to whatever the arm happens to say, which is how a declined payment becomes
`Pending` forever.

- [ ] Every connector status enum deserialized from an external payload has `#[serde(other)]`
- [ ] No `_ =>` arm in any `From<ConnectorStatus> for AttemptStatus` / `RefundStatus` impl
- [ ] The `Unknown` arm maps to a **non-terminal** status (`Pending`) so a sync can resolve it later

---

## 4. Error Handling

- [ ] Use specific `IntegrationError` variants (`NotSupported`, `InvalidDataFormat`,
      `MissingRequiredField`, `CurrencyNotSupported`, ...) -- **verify the variant exists** with
      `rg -n 'pub enum IntegrationError' -A 200 crates/types-traits/domain_types/src/errors.rs`
      before using it. There is no `InvalidData` and no `InvalidCard`
- [ ] Every `IntegrationError` / `ConnectorError` variant is constructed **with its `context`
      field** (`context: Default::default()` at minimum) -- none of them can be written bare
- [ ] Request-phase errors are `IntegrationError`; response-phase errors are `ConnectorError`.
      `ConnectorError` has exactly five variants: `ResponseDeserializationFailed`,
      `ResponseHandlingFailed`, `UnexpectedResponseError`, `IntegrityCheckFailed`,
      `ConnectorErrorResponse(Box<ErrorResponse>)`
- [ ] Webhook methods return `error_stack::Report<WebhookError>`, not `IntegrationError`
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

- [ ] Use the framework amount conversion utilities from `common_utils::types`. There are **five**
      unit types: `StringMajorUnit`, `FloatMajorUnit`, `MinorUnit`, `StringMinorUnit`,
      `StringTwoDecimalUnit`
- [ ] The chosen unit **matches the vendor spec's wire format**, and the reviewer can see which
      spec sentence or sample payload it came from. `StringMinorUnit` is not a safe default -- only
      10 of 67 converter-declaring connectors use it
- [ ] Currency unit (Base vs Minor) is configured correctly in `ConnectorCommon` or `create_all_prerequisites!`
- [ ] Amount converter is set up in `create_all_prerequisites!` macro (or `amount_converters: []`
      with transformers calling `convert_amount(&<Unit>ForConnector, ..)` directly)
- [ ] Never implement custom currency conversion -- use `utils::to_currency_base_unit` and similar
- [ ] Verify zero-decimal currencies are handled correctly by the framework config

---

## 7. Authentication Pattern

- [ ] Auth type struct carries exactly the credentials the connector needs, extracted from the
      connector's own `ConnectorSpecificConfig` variant via `TryFrom`
- [ ] `build_headers` constructs auth headers from the auth type correctly
- [ ] API keys and secrets are sourced from `req.connector_config: ConnectorSpecificConfig` --
      never hardcoded. `RouterDataV2::connector_auth_type` was **deleted** (2026-03-14, `a7a696c3a`);
      referencing it is `E0609`, and `fn get_auth_header(&self, _: &ConnectorAuthType)` does not
      match the trait (`E0407`). The real signature is in
      `crates/types-traits/interfaces/src/api.rs:25`
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

## 9. Literals, Constants and Fallbacks

Every value that reaches the wire, or that stands in for a value the connector did not send, must
be traceable to something. Reviewers check this line by line.

**Every literal on the wire is one of three things** -- nothing else is acceptable:

1. A **named constant** (`common_utils::consts`, or a `const` in the connector module)
2. An **enum variant** (`common_enums::Currency`, `CountryAlpha2`, a connector-local enum)
3. **Derived from RouterData** (`req.request.currency`, `req.resource_common_data...`)

- [ ] No bare string or numeric literal in a request body except as the value of a named `const`
- [ ] No magic status codes, channel identifiers, or version strings inline -- name them
- [ ] Enum-shaped fields use an enum, not `String`

**Error code / message fallbacks use the named constants.** `unwrap_or_default()` on an error code
yields `""`, which is indistinguishable downstream from a connector that genuinely sent an empty
code, and it defeats every error-code dashboard. `NO_ERROR_CODE` alone appears 247 times in real
connectors;
GRACE's guides used them zero times before this pass.

```rust
use common_utils::consts::{NO_ERROR_CODE, NO_ERROR_MESSAGE};

// Wrong
code: response.error_code.unwrap_or_default(),
message: response.error_message.unwrap_or_default(),

// Correct
code: response.error_code.unwrap_or_else(|| NO_ERROR_CODE.to_string()),
message: response.error_message.unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
```

- [ ] No `.unwrap_or_default()` on an error code or error message
- [ ] `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` (`crates/common/common_utils/src/consts.rs`) used for
      absent connector error fields

**Externally-driven enums carry `#[serde(other)]`.** Any enum deserialized from a payload the
vendor controls -- statuses, event types, error categories -- needs an `Unknown` variant marked
`#[serde(other)]`, or the vendor adding a value breaks deserialization of an otherwise valid
response. (See section 3 for the matching status-mapping rule.)

- [ ] Every enum deserialized from a connector payload has `#[serde(other)] Unknown`

**A retained fallback carries a citation.** If a default value, a magic constant, or a
"if the connector omits X, assume Y" branch survives review, the line above it must say *why*, with
a one-line pointer to the vendor spec section or the OSS reference implementation it came from:

```rust
// Spec §4.2.1: `settlement_currency` is omitted when it equals the presentment
// currency, so falling back to request.currency is the documented behaviour.
let settlement_currency = response.settlement_currency.unwrap_or(req.request.currency);
```

- [ ] Every surviving fallback/default has a one-line spec or OSS citation directly above it
- [ ] A fallback with no citation is deleted, or the value is made required and its absence is an
      error

---

## 10. Failure Honesty

The worst class of connector bug is not a compile error -- it is reporting the wrong outcome for
real money. Three rules.

**In-band 2xx failure must return `Err(ErrorResponse { .. })`.** Many gateways return HTTP 200 with
a body that says the payment was declined. Deserializing that into a success response reports a
declined payment as authorized. Branch on a success predicate and construct the error side
explicitly:

```rust
let status = AttemptStatus::from(item.response.status);
let response = if utils::is_payment_failure(status) {
    Err(ErrorResponse {
        code: item.response.error_code.clone()
            .unwrap_or_else(|| NO_ERROR_CODE.to_string()),
        message: item.response.error_message.clone()
            .unwrap_or_else(|| NO_ERROR_MESSAGE.to_string()),
        reason: item.response.error_message.clone(),
        status_code: item.http_code,
        attempt_status: Some(FlowStatus::Payment(status)),
        connector_transaction_id: Some(item.response.id.clone()),
        ..Default::default()
    })
} else {
    Ok(PaymentsResponseData::TransactionResponse { /* all 11 fields */ })
};
```

`utils::is_payment_failure` (`crates/types-traits/domain_types/src/utils.rs:231`) and
`is_refund_failure` (`crates/integrations/connector-integration/src/utils.rs:301`) are the
framework predicates -- use them rather than re-deciding which statuses count as failure.

- [ ] Every `handle_response_v2` / response `TryFrom` branches on a success predicate and returns
      `Err(ErrorResponse{..})` for in-band failures, not `Ok(..)` with a failure status
- [ ] The predicate is `is_payment_failure` / `is_refund_failure`, or an explicit exhaustive match
      -- not a string comparison

**`attempt_status` on the shared error path must be flow-aware and non-terminal by default.**
`attempt_status` is `Option<FlowStatus>`, and `FlowStatus` is domain-tagged
(`Payment` / `Refund` / `Dispute` / `Payout`). Two symmetrical bugs:

- Hardcoding `Some(FlowStatus::Payment(AttemptStatus::Failure))` in a shared `build_error_response`
  marks a *refund* error with a *payment* status, and marks a transient error (a 502, a timeout) as
  a terminal failure. This is the bug that reports a charged payment as FAILURE.
- Blanket `attempt_status: None` is equally wrong: a hard-declined refund then stays `Pending` and
  the framework keeps retrying it forever.

The correct shape classifies the error first and only then picks a status, defaulting to
non-terminal. Minimal form -- `connectors/noon.rs:499-512`:

```rust
// Only this one connector-specific code is known-terminal; everything else stays open.
let attempt_status = if response.result_code == 19001 {
    Some(AttemptStatus::Failure)
} else {
    None
};
// ...
attempt_status: attempt_status.map(FlowStatus::Payment),
```

Full form, classifying refund vs payment -- `connectors/flywire.rs:362-370`:

```rust
let attempt_status: FlowStatus = if is_refund_failure {
    FlowStatus::Refund(RefundStatus::Failure)
} else {
    let payment_status = match response.status {
        Some(401) | Some(403) => AttemptStatus::AuthenticationFailed,
        Some(404) | Some(422) => AttemptStatus::Failure,
        _ => AttemptStatus::Pending,          // transient: leave it open for PSync
    };
    FlowStatus::Payment(payment_status)
};
```

- [ ] `attempt_status` on a shared error path never crosses domains (a refund flow never emits
      `FlowStatus::Payment`, and vice versa)
- [ ] The default branch is **non-terminal** (`Pending`, or `None`) -- terminal statuses are set
      only for errors the connector documents as final
- [ ] No unconditional `attempt_status: Some(AttemptStatus::Failure)` anywhere (it is also a type
      error now -- the field is `Option<FlowStatus>`)

**Per-flow terminal states are per-flow.** A Capture that fails is `CaptureFailed`, not `Failure`;
a Void that fails is `VoidFailed`; an authorization that is declined is `AuthorizationFailed`. Using
the generic `Failure` for all of them loses the information the operator needs to decide whether
the money moved.

- [ ] Capture failures map to `AttemptStatus::CaptureFailed`
- [ ] Void failures map to `AttemptStatus::VoidFailed`
- [ ] Authorization declines map to `AttemptStatus::AuthorizationFailed`
- [ ] Refund failures map to `RefundStatus::Failure` / `TransactionFailure`, never to an
      `AttemptStatus`

---

## 11. Common Mistakes to Avoid

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
| `error_code.unwrap_or_default()` | `.unwrap_or_else(\|\| NO_ERROR_CODE.to_string())` |
| `attempt_status: Some(AttemptStatus::Failure)` on the shared error path | Classify the error, then `attempt_status.map(FlowStatus::Payment)` / `FlowStatus::Refund`; default non-terminal |
| `_ =>` catch-all in a status `From` impl | Exhaustive match + `#[serde(other)] Unknown` on the enum |
| Defaulting to `StringMinorUnit` when the spec is unclear | Read the vendor's sample payload; the plurality of connectors use `StringMajorUnit` |
| `impl SourceVerification<Flow, Data, Req, Resp>` per flow | One non-generic `impl SourceVerification for {Conn}<T> {}` (`E0107`) |
| `req.connector_auth_type` | `req.connector_config: ConnectorSpecificConfig` (field deleted `a7a696c3a`) |
| `request.payment_amount` on Capture | No such field; use `amount_to_capture` (bare `i64`) / `minor_amount_to_capture` |
| `ConnectorError::InvalidData` / `::NotImplemented` / `::InvalidCard` | None exist; pick a real `IntegrationError` variant and keep its `context` |
| `transformation_status` on a webhook response | Field does not exist (`E0560`) -- delete it |
| Missing `macro_connector_flow_status_impls!` | Add it; 112/112 connectors need it or `E0277` |
| 2xx response with a declined body returned as `Ok(..)` | Branch on `is_payment_failure` and return `Err(ErrorResponse{..})` |

---

## 12. Macro Implementation Checks

- [ ] All implemented flows defined in `create_all_prerequisites!` macro
- [ ] All implemented flows use `macro_connector_implementation!` -- no manual trait impls
- [ ] **`macro_connector_flow_status_impls!` is present** and covers every flow marker the
      connector does not implement, split correctly between `not_implemented:` (vendor supports it,
      we have not built it) and `not_supported:` (vendor does not offer it). 112 of 112 connectors
      at HEAD invoke this macro; omitting a flow is `E0277`
- [ ] `macro_connector_payout_implementation!` is present unless the connector implements payout
      flows by hand
- [ ] A flow with no outbound HTTP call uses `macro_connector_local_flow_implementation!` and is
      **not** also listed in `macro_connector_flow_status_impls!`
- [ ] Exactly **one** non-generic `impl SourceVerification for {Connector}<T>` and one
      `impl BodyDecoding for {Connector}<T>` -- never one per flow (`E0107`)
- [ ] `build_error_response` / `get_error_response_v2` / `get_5xx_error_response` each take the
      **third** `&ConnectorSpecificConfig` parameter, and the event-builder type is `events::Event`
      (not `ConnectorEvent`). No calls to `set_error_response_body`
- [ ] HTTP methods match the connector API documentation (GET, POST, PUT, DELETE)
- [ ] Content types are correct (Json, FormData, FormUrlEncoded, or omitted)
- [ ] GET endpoints omit `curl_request` parameter; POST/PUT endpoints include it
- [ ] `member_functions` includes `build_headers` and `connector_base_url`
- [ ] Amount converter configured when the flow handles monetary amounts

---

## 13. Cross-Flow Consistency

- [ ] All flows use the same authentication pattern
- [ ] Shared types (status enums, error structs) are defined once and reused
- [ ] Similar operations are implemented similarly across flows
- [ ] Transformer logic is reused where applicable (shared helper functions)
- [ ] Naming style is uniform across all flow files

---

## 14. Final Verification Steps

Run these checks before declaring the connector complete:

1. **Build**: `cargo build` -- must pass cleanly
2. **Architecture**: Grep for `RouterData<` (not V2), `ConnectorIntegration<` (not V2), `hyperswitch_domain_models` -- all must return zero results in your connector files
3. **Hardcoded status**: Search for direct `AttemptStatus::Charged`, `AttemptStatus::Failure` etc. outside of a match arm mapping from connector response -- must be zero
4. **Dead fields**: Check every `None` assignment in request builders -- verify the field is conditionally used, not always None
5. **Error quality**: Verify every `NotSupported` error includes the specific unsupported item name
6. **Completeness**: Confirm all six core flows are implemented (Authorize, PSync, Capture, Refund, RSync, Void) plus any pre-auth flows required by the connector
7. **Status coverage**: Verify every status value documented in the connector's API spec has a mapping
8. **Struct cleanliness**: No unused fields, no unnecessary Option wrappers, no placeholder values
9. **Fallbacks**: `rg -n 'unwrap_or_default\(\)' <your files>` -- zero hits on error code/message.
   Every remaining `unwrap_or` / `unwrap_or_else` has a spec citation above it
10. **Unknown statuses**: `rg -n 'serde\(other\)' <your transformers>` -- one hit per
    externally-driven enum; `rg -n '_ =>' <your transformers>` -- zero hits inside status `From`
    impls
11. **Failure honesty**: `rg -n 'AttemptStatus::Failure' <your files>` -- every hit is inside a
    match arm on a documented terminal connector status, never on a shared error path
12. **Stub coverage**: `rg -n 'macro_connector_flow_status_impls' <your connector>.rs` -- one hit,
    and its two lists plus your implemented flows account for every marker in
    `crates/types-traits/domain_types/src/connector_flow.rs`

---

## 15. Pre-Flight Gate (run last, before pushing)

Sections 1–14 are written as rules. This section is written as **tests**. Every item has a
mechanical check whose output you can paste into the PR; "I looked and it seemed fine" is
not a pass. Run these against the final diff, on the branch head, after the last fix
commit.

Set `FILES` once and reuse it:

```bash
FILES="crates/integrations/connector-integration/src/connectors/<name>.rs \
crates/integrations/connector-integration/src/connectors/<name>/"
```

### 15.1 No literal on the wire

Every value that leaves the process is a **named constant**, an **enum variant**, or
**derived from RouterData**. Nothing else. (Section 9 states the rule; this is how you
prove it.)

```bash
# Candidate literals inside request construction. Every hit must be justified below.
rg -n '"[^"]*"' $FILES | rg -v '^\S+:\s*//' | rg -v 'const |#\[|rename|serde|expect\(|attach_printable|field_name|\.to_string\(\) *$'
```

**Passes only when** for every surviving hit you can name which of the three categories it
falls into. In practice: a `const` declaration site is fine, a `#[serde(rename = "...")]`
is fine (that is the wire *name*, not a wire *value*), and a bare string being assigned to
a request field is not.

- [ ] Every string/numeric literal reaching a request body, URL, or header is a `const`, an
      enum variant, or read off `req`
- [ ] Enum-shaped fields (channel, transaction type, API version, capture mode) are typed
      as enums, not `String`

### 15.2 `#[serde(other)]` on every externally-driven enum

An enum deserialized from a payload the *vendor* controls needs an `Unknown` variant marked
`#[serde(other)]`, or the vendor adding one value breaks deserialization of an otherwise
valid response.

```bash
# Every enum that is deserialized:
rg -n -B2 'enum ' $FILES | rg -n 'Deserialize'
# Every escape hatch:
rg -n 'serde\(other\)' $FILES
```

**Passes only when** the second list has one entry for each enum in the first list that is
built from a connector response. Enums used only in *requests* (values you choose) do not
need it — and must not have it, since `Unknown` would then be serializable.

- [ ] Every response-side enum has `#[serde(other)] Unknown`
- [ ] The status-mapping `match` remains exhaustive over named variants — `rg -n '_ =>' $FILES`
      returns zero hits inside a status `From`/`TryFrom` impl (section 3)

### 15.3 No silent fallback

Every `unwrap_or`, `unwrap_or_else`, `unwrap_or_default` applied to a **wire value or a
status** either carries a one-line citation directly above it, or is deleted and the
absence made an error.

```bash
rg -n -B1 'unwrap_or(_else|_default)?\(' $FILES
```

**Passes only when** each hit is one of:

1. `NO_ERROR_CODE` / `NO_ERROR_MESSAGE` on an error code/message (section 9), or
2. preceded by a comment naming the vendor spec section or OSS reference that documents the
   default, e.g.
   `// Spec §4.2.1: settlement_currency is omitted when it equals the presentment currency.`

Anything else becomes an error instead. The framework idiom is
`.ok_or_else(missing_field_err("<field>"))` / `IntegrationError::MissingRequiredField` —
see `extract_merchant_identifiers_from_metadata` in
`connectors/juspay_upi_stack/transformers.rs`, which errors on every missing piece rather
than substituting one.

`rg -n 'unwrap_or_default\(\)' $FILES` must return **zero** hits on an error code or
message.

- [ ] Every `unwrap_or*` on a wire or status value has a citation, or has become an `Err`
- [ ] Zero `unwrap_or_default()` on error code / error message

### 15.4 An in-band failure returns `Err`

A gateway that answers HTTP 200 with a declined body must not produce
`Ok(PaymentsResponseData::…)`.

```bash
rg -n 'is_payment_failure|is_refund_failure' $FILES
rg -n 'fn handle_response_v2' $FILES
```

**Passes only when** every `handle_response_v2` (and every response `TryFrom`) branches on
a success predicate and constructs `Err(ErrorResponse { .. })` on the failure side.
Predicates are `utils::is_payment_failure` (`domain_types/src/utils.rs`) and
`is_refund_failure` (`connector-integration/src/utils.rs`) — not a string comparison, and
not a re-decision of which statuses count as failure. Section 10 has the full shape.

- [ ] Every response handler has a failure branch returning `Err(ErrorResponse{..})`
- [ ] The predicate is the framework one, or an explicit exhaustive match
- [ ] `attempt_status` on a shared error path is flow-aware and non-terminal by default

### 15.5 PSync does not re-derive a lookup key it could have read

This is the item that survives a careless review, so read the phrasing exactly: the check
is **not** "does PSync read the carrier" — a fallback makes that check pass while the bug
is fully intact.

The carrier chain is:

```
Authorize response  ->  PaymentsResponseData::TransactionResponse { connector_metadata }
                    ->  PaymentFlowData.connector_feature_data
                    ->  gRPC connector_feature_data
```

`PaymentsSyncData` has **no** `connector_meta` field. `get_connector_meta()` is an
accessor on `PaymentFlowData` that simply calls `get_connector_feature_data()` (see
`impl PaymentFlowData` in `domain_types/src/connector_types.rs`), and both
`PaymentFlowData` and `PaymentsSyncData` carry a `connector_feature_data:
Option<SecretSerdeValue>`. It is also **not** `encoded_data` — the authorize response has
no such field.

```bash
# Every identifier PSync puts into a URL, header or body:
rg -n -A25 'RouterDataV2<PSync' $FILES

# The hazard: a read of the carrier that degrades instead of failing.
rg -n 'connector_feature_data|get_connector_meta' -A4 $FILES | rg 'unwrap_or|ok\(\)|and_then|\.unwrap\('

# The other hazard: rebuilding an identifier the connector already handed back.
rg -n 'format!|uuid|Uuid::|now\(\)|timestamp' $FILES | rg -i 'psync|sync'
```

**Passes only when**:

- For every identifier in the PSync request you can name the field it was **read** from —
  `req.request.connector_transaction_id`, `req.resource_common_data.connector_feature_data`,
  `req.resource_common_data.get_connector_meta()?` — and
- the second command returns **zero** hits. A read of the carrier followed by `unwrap_or(…)`
  or `.ok()` reports as "PSync reads the carrier" and then silently syncs the wrong
  transaction the moment the carrier is absent. A missing carrier must be
  `IntegrationError::MissingRequiredField`, and
- the third command returns no identifier that the Authorize response already contained.
  Re-deriving `merchant_reference`, an order id, or a timestamp-based key produces a value
  the gateway never saw.

Working exemplar: `connectors/axisbank.rs`, `get_headers` for
`RouterDataV2<PSync, PaymentFlowData, PaymentsSyncData, PaymentsResponseData>` —
`extract_merchant_identifiers_from_metadata(&req.resource_common_data.connector_feature_data)?`
with the `?`, no fallback, and `connector_transaction_id` read via
`get_connector_transaction_id()` mapped to `MissingRequiredField`.

- [ ] Every PSync identifier is read, not reconstructed
- [ ] No `unwrap_or` / `.ok()` / `.unwrap()` on the carrier read — absence is an error
- [ ] RSync gets the same treatment (`req.request.connector_refund_id`)

### 15.6 Evidence is regenerated against the branch head

Request/response captures, grpcurl transcripts and certification reports go stale the
moment a fix commit lands. Evidence taken before the last fix is evidence for code that is
not being merged.

```bash
git log --oneline "$(git merge-base origin/main HEAD)..HEAD"   # last commit = the one evidenced
git status --porcelain                                          # must be empty
```

**Passes only when** the newest evidence in the PR body was produced *after* the newest
commit on the branch. If you fixed anything in response to review, regenerate — do not
edit the old transcript.

- [ ] Every capture / transcript in the PR body was produced against the current branch head
- [ ] The working tree is clean (nothing evidenced that is not committed)
- [ ] `cargo run --bin check_connector_specs` was re-run after the last commit —
      see `.skills/_shared/references/certification.md` §1

---

## 16. What the PR body must disclose

GRACE does **not** write tests. That is a standing decision and this section does not lift
it. It does mean the PR body is the only place a human learns that something in the diff
needs a test they will have to write.

**Novel algorithmic logic must be listed in the PR body**, one line each, so a reviewer can
add a known-answer test. "Novel" means logic whose correctness is not visible by reading it
— anything with a right answer that a wrong implementation would still compile and often
still *run*:

- **Signing / MAC**: HMAC construction, the exact canonical string, field order, separator,
  encoding of the digest (hex vs base64, upper vs lower)
- **Hashing / digests**: SHA-256 of a body, a content digest header, a nonce derivation
- **Checksums**: Luhn, mod-97, a vendor's own check digit
- **Custom amount encoding**: anything beyond the five `*ForConnector` amount converters in
  `crates/common/common_utils/src/types.rs` (`MinorUnitForConnector`,
  `StringMinorUnitForConnector`, `StringMajorUnitForConnector`, `FloatMajorUnitForConnector`,
  `StringTwoDecimalUnitForConnector`) — zero-decimal currency tables, implicit
  decimal places, amounts as strings with a fixed width
- **Timestamp / nonce formats** that feed a signature
- **Any bespoke serialization** the vendor requires (sorted query strings, form encoding
  with a specific escaping rule)

Use this shape, so the reviewer knows exactly what to pin:

```markdown
### Needs a known-answer test (not written here — GRACE does not write tests)

- `signature.rs::sign_request` — HMAC-SHA256 over
  `"{method}\n{path}\n{timestamp}\n{body}"`, digest lower-hex. Spec §6.3.
  Suggested vector: the worked example in §6.3.1 of the vendor doc.
- `transformers.rs::to_vendor_amount` — implicit-2-decimal integer for all
  currencies except JPY/KRW (zero-decimal). Spec Appendix B.
```

- [ ] Every signing / hashing / checksum / custom-amount-encoding routine in the diff is
      listed in the PR body with the spec section it implements
- [ ] Each listed item names a concrete input → expected output the reviewer can use as the
      test vector (the vendor's own worked example, when there is one)
- [ ] No test files were added (section is a disclosure requirement, not a test requirement)

### PR body checklist

- [ ] Certification status stated: credentials + passing scenarios, **or** the
      `alpha_connectors.json` entry and its `reason` — `.skills/_shared/references/certification.md` §3
- [ ] `cargo run --bin check_connector_specs` output pasted (`All checks passed. OK.`)
- [ ] `supported_suites` in `specs.json` matches what is actually implemented
- [ ] Novel algorithmic logic listed as above
- [ ] Evidence regenerated against the branch head (§15.6)
