# Subagent Prompts — new-connector

Each step in the new-connector workflow can be delegated to an independent subagent.
The orchestrator (SKILL.md) coordinates the sequence and passes outputs between them.

---

## Subagent 1: Tech Spec Validation

**Inputs**: connector_name
**Outputs**: extracted config (name, base_url, auth, amount, content_type, flows, pre-auth flows)

```
Validate the tech spec for the {ConnectorName} connector.

Read: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
  (also check: grace/rulesbook/codegen/references/specs/{Connector_Name}.md — the specs/
   directory is keyed by the exact-casing connector name, e.g. Dlocal.md, T-kassa.md)

Extract and report:
1. Connector name: snake_case and PascalCase forms
2. Base URL for the API
3. Authentication method (API key / Basic Auth / OAuth / Bearer token)
4. Amount format -- read the vendor spec's wire format and match it. There is no safe
   default. The five types in crates/common/common_utils/src/types.rs:
     MinorUnit            integer minor units, e.g. 1050
     StringMinorUnit      string minor units, e.g. "1050"
     StringMajorUnit      string major units, e.g. "10.50"
     FloatMajorUnit       float major units, e.g. 10.50
     StringTwoDecimalUnit string major units always at 2dp, e.g. "10.50"
5. Content type (JSON / form-encoded / XML)
6. For each flow (Authorize, Capture, Refund, Void, PSync, RSync):
   - HTTP method (POST/GET/PUT)
   - Endpoint URL path
   - Key request fields
   - Status values returned
7. Pre-auth flow detection — report using the flow-marker names from
   crates/types-traits/domain_types/src/connector_flow.rs (there is no CreateAccessToken or
   CreateSessionToken in the codebase). Check if the spec mentions:
   - ServerAuthenticationToken: OAuth/token auth (POST /login, /oauth/token, /auth) → YES/NO
   - ServerSessionAuthenticationToken: session init before payment → YES/NO
   - CreateOrder: order/intent creation before payment → YES/NO
   - CreateConnectorCustomer: customer object required before payment → YES/NO
   - PaymentMethodToken: tokenization before authorize → YES/NO

If the tech spec is missing → IMMEDIATELY return FAILED. Do NOT continue.
Reason: "Tech spec not found. Run generate-tech-spec skill first, or provide the
tech spec manually. Cannot proceed without a tech spec — do NOT infer API details
from any other source."

Output format:
  CONNECTOR: {ConnectorName}
  BASE_URL: ...
  AUTH: HeaderKey | SignatureKey | BodyKey
  AMOUNT: MinorUnit | StringMinorUnit | StringMajorUnit | FloatMajorUnit | StringTwoDecimalUnit
  CONTENT_TYPE: Json | FormUrlEncoded | Xml
  CORE_FLOWS: [Authorize, PSync, Capture, Refund, RSync, Void]
  PRE_AUTH_FLOWS: [none] or [ServerAuthenticationToken, ...]
  STATUS: SUCCESS | FAILED
```

---

## Subagent 2: Foundation Setup

**Inputs**: connector_name, base_url, production_base_url
**Outputs**: scaffold created, superposition URLs registered + URL patching wired, connector_specs/<name>/specs.json written, build passes, convention check results

```
Set up the foundation for the {ConnectorName} connector.

1. Run the scaffold script:
   .skills/new-connector/scripts/add_connector.sh {connector_name} {base_url} --force -y

   That path is a symlink to the real script, grace/rulesbook/codegen/add_connector.sh —
   either path works. There is NO scripts/add_connector.sh at the repo root.

   If the production base URL differs from the sandbox {base_url}, pass it too so the
   superposition production override is correct:
   .skills/new-connector/scripts/add_connector.sh {connector_name} {base_url} --production-url {production_base_url} --force -y

   The script also writes crates/internal/integration-tests/src/connector_specs/{connector_name}/specs.json
   (required by CI's `cargo run --all-features --bin check_connector_specs`), seeding
   supported_suites from --flows. The default is the six core flows
   (Authorize,PSync,Capture,Void,Refund,RSync), so do NOT pass --flows unless this connector
   needs a different set:
   .skills/new-connector/scripts/add_connector.sh {connector_name} {base_url} --flows Authorize,PSync,Capture,Void,Refund,RSync,SetupMandate --force -y

   --flows takes check_connector_specs flow names (the flow_to_suite table in the script),
   NOT the trait names `--list-flows` prints. Accepted: Authorize, PSync, Capture, Void, Refund,
   RSync, SetupMandate, RepeatPayment, MandateRevoke, CreateConnectorCustomer,
   GetConnectorCustomer, PaymentMethodToken, PaymentMethodEligibility, ServerAuthenticationToken,
   ClientAuthenticationToken, ServerSessionAuthenticationToken, PreAuthenticate, Authenticate,
   PostAuthenticate, CreateOrder, IncrementalAuthorization. An unrecognised name aborts the run.

2. Verify the build:
   cargo build --package connector-integration

3. Open the generated files and verify UCS conventions:
   - Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
   - Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs
   - Registry: crates/integrations/connector-integration/src/connectors.rs (has pub mod {connector_name})

4. Convention checks (fix any violations):
   - Struct is {ConnectorName}<T> (generic), not {ConnectorName}
   - Uses RouterDataV2, not RouterData
   - Uses ConnectorIntegrationV2, not ConnectorIntegration
   - Imports from domain_types, not hyperswitch_domain_models

5. Set up the amount converter:
   macros::create_amount_converter_wrapper!(connector_name: {ConnectorName}, amount_type: {AmountType});

6. Implement ConnectorCommon trait:
   - id() returns "{connector_name}"
   - common_get_content_type() returns "application/json" (or correct type)
   - base_url() returns connectors.{connector_name}.base_url.as_ref()
   - get_auth_header() extracts auth from ConnectorSpecificConfig::{ConnectorName}.
     The trait signature is
       fn get_auth_header(&self, auth_type: &ConnectorSpecificConfig)
           -> CustomResult<Vec<(String, Maskable<String>)>, errors::IntegrationError>
     (crates/types-traits/interfaces/src/api.rs:25). RouterDataV2 no longer has a
     connector_auth_type field; a get_auth_header(&ConnectorAuthType) is E0407.
     Copy the idiom from a recent connector, e.g. connectors/travelhub.rs.
   - build_error_response() parses connector error format. THREE parameters:
       fn build_error_response(
           &self,
           res: domain_types::router_response_types::Response,
           _event_builder: Option<&mut events::Event>,
           _connector_config: &ConnectorSpecificConfig,
       ) -> CustomResult<ErrorResponse, errors::ConnectorError>
     (interfaces/src/api.rs:50). The event type is events::Event -- there is no
     ConnectorEvent in this crate, and events::Event has no set_error_response_body method.
     ErrorResponse has 13 fields and implements Default, so build it with
     ..Default::default(). attempt_status is Option<FlowStatus>, NOT Option<AttemptStatus>:
     wrap as Some(FlowStatus::Payment(AttemptStatus::...)) — and do not force a terminal
     status on this shared path (see connectors/flywire.rs:362-370). Use
     NO_ERROR_CODE / NO_ERROR_MESSAGE (crates/common/common_utils/src/consts.rs) as the
     fallbacks, never .unwrap_or_default().

7. VERIFY (do NOT re-add) the base trait markers the scaffold already emitted:
   - connector_types::ConnectorServiceTrait<T>, ValidationTrait, IncomingWebhook and
     VerifyRedirectResponse, plus interfaces::verification::SourceVerification — all written
     by add_connector.sh
   - BodyDecoding — written by template-generation/connector.rs.template
   Writing any of these a second time is a conflicting implementation (E0119), not a no-op.
   SourceVerification and BodyDecoding are NON-generic traits (interfaces/src/verification.rs,
   interfaces/src/decode.rs): exactly one impl per connector, never one per flow. A per-flow
   impl<T> SourceVerification<Flow, Data, Req, Resp> is E0107. Exemplar: travelhub.rs:175.
   Also confirm macro_connector_flow_status_impls! and macro_connector_payout_implementation!
   were emitted at the end of the connector file.

8. VERIFY superposition URL registration + dynamic URL patching (the scaffold script in step 1
   now does BOTH of these automatically — confirm they landed; do them by hand only if missing).
   Naming: superposition enum value / _context_ / patched.<field> use snake_case
   ({connector_name}); ConnectorEnum::<Variant> uses PascalCase ({ConnectorName}).

   a. config/superposition.toml
      - "{connector_name}" is in the `connector` dimension enum under [dimensions].
      - Override blocks exist at the END of the file (sandbox default + production):

        # {ConnectorName}
        [[overrides]]
        _context_ = { connector = "{connector_name}" }
        connector_base_url = "{sandbox_base_url}"

        # {ConnectorName} Production
        [[overrides]]
        _context_ = { connector = "{connector_name}", environment = "production" }
        connector_base_url = "{production_base_url}"

      - If you did NOT pass --production-url, the production override reuses {base_url}; fix it if
        the connector has a distinct live URL.

   b. crates/types-traits/domain_types/src/types.rs  ->  Connectors::patch_connector_urls()
      - A match arm exists BEFORE the `_ =>` fallback:

        ConnectorEnum::{ConnectorName} => {
            patched.{connector_name}.apply(params_patch);
        }

      - "{connector_name}" is in the "Supported connectors:" list in the `_ =>` error message.

9. VERIFY the CI spec file landed:
   crates/internal/integration-tests/src/connector_specs/{connector_name}/specs.json exists and its
   supported_suites list is non-empty. Without it, CI's check_connector_specs job fails.

10. Verify: cargo build --package connector-integration

Output:
  STATUS: SUCCESS | FAILED
  FILES_CREATED: [list of files]
  FILES_MODIFIED: [config/superposition.toml, crates/types-traits/domain_types/src/types.rs, ...]
  SUPERPOSITION_URLS_REGISTERED: YES | NO
  URL_PATCHING_WIRED: YES | NO
  CONNECTOR_SPECS_JSON: crates/internal/integration-tests/src/connector_specs/{connector_name}/specs.json WRITTEN | MISSING
  BUILD: PASS | FAIL
  CONVENTION_VIOLATIONS: [none] or [list]
```

---

## Subagent 3: Flow Implementation (per flow)

**Inputs**: connector_name, flow_name, tech_spec_path
**Outputs**: flow implemented, build passes

See `flow-implementation-guide.md` for the complete procedure and prompt template.

```
Implement the {FlowName} flow for {ConnectorName}.

Tech spec: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
Pattern: .skills/new-connector/references/flow-patterns/{flow}.md
Macro ref: .skills/new-connector/references/macro-reference.md
Implementation guide: .skills/new-connector/references/flow-implementation-guide.md
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Instructions:
1. Read the tech spec for {FlowName} endpoint details
2. Read the flow pattern file for {FlowName}-specific patterns
3. Read the implementation guide for the 3-part procedure
4. FIRST: remove {FlowName}'s marker name from the not_implemented: [...] list in the
   macro_connector_flow_status_impls! invocation at the bottom of the connector file.
   That macro (macros.rs ~:1827) emits BOTH the marker-trait impl and a stub
   ConnectorIntegrationV2 impl for every flow it lists, so leaving the name there while
   adding your own is a double E0119.
5. Add flow to create_all_prerequisites! macro
6. Add macro_connector_implementation! block
7. Create request/response types and TryFrom impls in transformers.rs
8. Add the flow's trait marker impl, now freed by step 4 (marker names are not uniform;
   check the flow-implementation-guide.md type table)
9. Run: cargo build --package connector-integration
10. Fix compilation errors

Output:
  FLOW: {FlowName}
  STATUS: SUCCESS | FAILED
  BUILD: PASS | FAIL
  REASON: (if failed)
```

---

## Subagent 4: gRPC Testing (per flow or all flows)

**Inputs**: connector_name, flows_to_test, creds_path
**Outputs**: test results per flow

See `grpc-testing-guide.md` for the complete procedure and prompt template.

```
Test the {ConnectorName} connector flows via grpcurl.

Testing guide: .skills/new-connector/references/grpc-testing-guide.md
Credentials: creds.json (field: {connector_name})
Connector source: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Flows to test (in order): {flow_list}

Instructions:
1. Read the testing guide
2. Start the gRPC server if not running
3. Load credentials from creds.json
4. For each flow, run the grpcurl test using the correct service/method
5. Validate response against PASS/FAIL criteria
6. If FAILED: read server logs, diagnose, fix code, rebuild, retest (max 7 iterations)
7. Report results per flow

Output:
  CONNECTOR: {ConnectorName}
  RESULTS:
    Authorize: PASS | FAIL
    PSync: PASS | FAIL
    Capture: PASS | FAIL
    Refund: PASS | FAIL
    RSync: PASS | FAIL
    Void: PASS | FAIL
  STATUS: ALL_PASS | PARTIAL | ALL_FAIL
```

---

## Subagent 5: Quality Review

**Inputs**: connector_name
**Outputs**: quality score, violations found

```
Perform a quality review of the {ConnectorName} connector implementation.

Quality checklist: .skills/new-connector/references/quality-checklist.md
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Checks:
1. Architecture compliance:
   - grep for "RouterData<" (without V2) in connector files → must be 0
   - grep for "ConnectorIntegration<" (without V2) → must be 0
   - grep for "hyperswitch_domain_models" → must be 0

2. Status mapping:
   - No hardcoded AttemptStatus::Charged, AttemptStatus::Failure outside match arms
   - Every status from the connector API has a mapping in the From impl
   - Refund flows use RefundStatus, payment flows use AttemptStatus
   - The wire enum carries #[serde(other)] Unknown (deserialization layer) AND the
     status-mapping match has no catch-all `_ =>` (mapping layer). Reviewers demand both.
   - An in-band failure delivered with HTTP 2xx returns Err(ErrorResponse { .. }); branch on
     a success predicate (see utils::is_payment_failure in domain_types/src/utils.rs)

3. Code quality:
   - No unwrap() calls
   - No fields hardcoded to None (remove unused fields instead)
   - No unnecessary Option wrappers
   - All error messages are descriptive (include connector name)
   - No unnecessary .clone() calls
   - Error code/message fallbacks use NO_ERROR_CODE / NO_ERROR_MESSAGE
     (crates/common/common_utils/src/consts.rs), never .unwrap_or_default()
   - attempt_status on the shared error path is flow-aware, not a blanket
     Some(FlowStatus::Payment(AttemptStatus::Failure)) and not a blanket None
     (exemplar connectors/flywire.rs:362-370, minimal form connectors/noon.rs:499-512)
   - Only the five real ConnectorError variants appear (ResponseDeserializationFailed,
     ResponseHandlingFailed, UnexpectedResponseError, IntegrityCheckFailed,
     ConnectorErrorResponse), each with its context field. NotImplemented is not one of them —
     it belongs to IntegrationError, which is where request-side failures go. InvalidData and
     InvalidCard are variants of NEITHER enum; substitute a real IntegrationError variant
     (InvalidDataFormat, InvalidWallet, MismatchedPaymentData, ...) and keep its context field.

4. Macro completeness:
   - Every implemented flow in create_all_prerequisites! also has
     macro_connector_implementation!
   - Every implemented flow has its trait marker implementation, and its marker name is
     ABSENT from macro_connector_flow_status_impls!'s not_implemented list
   - Every unimplemented flow is still listed in macro_connector_flow_status_impls!
   - No duplicated base-trait impls (ConnectorServiceTrait / ValidationTrait /
     IncomingWebhook / VerifyRedirectResponse / SourceVerification / BodyDecoding appear
     exactly once each) -- a duplicate is E0119
   - ConnectorCommon trait is implemented

5. Naming conventions:
   - Request types: {ConnectorName}{Flow}Request
   - Response types: {ConnectorName}{Flow}Response
   - Status enums: {ConnectorName}{Flow}Status
   - Auth type: {ConnectorName}AuthType

6. Final build:
   cargo build --package connector-integration → must pass

Output:
  CONNECTOR: {ConnectorName}
  VIOLATIONS: [list] or [none]
  STATUS: PASS | FAIL
```
