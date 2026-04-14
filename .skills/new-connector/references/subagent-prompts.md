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
  (also check: grace/rulesbook/codegen/references/specs/{connector_name}.md)

Extract and report:
1. Connector name: snake_case and PascalCase forms
2. Base URL for the API
3. Authentication method (API key / Basic Auth / OAuth / Bearer token)
4. Amount format (integer cents = MinorUnit, string cents = StringMinorUnit, string dollars = StringMajorUnit)
5. Content type (JSON / form-encoded / XML)
6. For each flow (Authorize, Capture, Refund, Void, PSync, RSync):
   - HTTP method (POST/GET/PUT)
   - Endpoint URL path
   - Key request fields
   - Status values returned
7. Pre-auth flow detection — check if the spec mentions:
   - CreateAccessToken: OAuth/token auth (POST /login, /oauth/token, /auth) → YES/NO
   - CreateOrder: order/intent creation before payment → YES/NO
   - CreateConnectorCustomer: customer object required before payment → YES/NO
   - PaymentMethodToken: tokenization before authorize → YES/NO
   - CreateSessionToken: session init before payment → YES/NO

If the tech spec is missing → IMMEDIATELY return FAILED. Do NOT continue.
Reason: "Tech spec not found. Run generate-tech-spec skill first, or provide the
tech spec manually. Cannot proceed without a tech spec — do NOT infer API details
from any other source."

Output format:
  CONNECTOR: {ConnectorName}
  BASE_URL: ...
  AUTH: HeaderKey | SignatureKey | BodyKey
  AMOUNT: MinorUnit | StringMinorUnit | StringMajorUnit
  CONTENT_TYPE: Json | FormUrlEncoded | Xml
  CORE_FLOWS: [Authorize, PSync, Capture, Refund, RSync, Void]
  PRE_AUTH_FLOWS: [none] or [CreateAccessToken, ...]
  STATUS: SUCCESS | FAILED
```

---

## Subagent 2: Foundation Setup

**Inputs**: connector_name, base_url
**Outputs**: scaffold created, build passes, convention check results

```
Set up the foundation for the {ConnectorName} connector.

1. Run the scaffold script:
   .skills/new-connector/scripts/add_connector.sh {connector_name} {base_url} --force -y

   If the script doesn't exist there, also check:
   grace/rulesbook/codegen/add_connector.sh

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
   - get_auth_header() extracts auth from ConnectorSpecificConfig::{ConnectorName}
   - build_error_response() parses connector error format

7. Add required trait markers:
   - connector_types::ConnectorServiceTrait<T>
   - SourceVerification
   - BodyDecoding

8. Verify: cargo build --package connector-integration

Output:
  STATUS: SUCCESS | FAILED
  FILES_CREATED: [list of files]
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
4. Add flow to create_all_prerequisites! macro
5. Add macro_connector_implementation! block
6. Create request/response types and TryFrom impls in transformers.rs
7. Add trait marker if needed (check flow-implementation-guide.md type table)
8. Run: cargo build --package connector-integration
9. Fix compilation errors

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

3. Code quality:
   - No unwrap() calls
   - No fields hardcoded to None (remove unused fields instead)
   - No unnecessary Option wrappers
   - All error messages are descriptive (include connector name)
   - No unnecessary .clone() calls

4. Macro completeness:
   - Every flow in create_all_prerequisites! also has macro_connector_implementation!
   - Every flow has its trait marker implementation
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
