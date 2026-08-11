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
6. For each flow (Authorize, Capture, Refund, Void, PSync, RSync), check if it is documented in the tech spec with:
   - HTTP method (POST/GET/PUT)  
   - Endpoint URL path
   - Key request fields
   - Status values returned
   
   Only include flows that have COMPLETE documentation (all fields above). Skip any flow that is missing endpoint details or is marked as "N/A" or "Not Supported".
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
  CORE_FLOWS: [list only flows documented in tech spec, e.g., Authorize, Capture] or [none]
  PRE_AUTH_FLOWS: [none] or [CreateAccessToken, ...]
  STATUS: SUCCESS | FAILED
  
IMPORTANT: Only include flows in CORE_FLOWS if they have complete endpoint documentation in the tech spec. If a flow section is missing, incomplete, or marked as "N/A", do NOT include it in the list.
```

---

## Subagent 2: Foundation Setup

**Inputs**: connector_name, base_url
**Outputs**: scaffold created, convention check results

```
Set up the foundation for the {ConnectorName} connector.

1. Run the scaffold script:
   .skills/new-connector/scripts/add_connector.sh {connector_name} {base_url} --force -y

   If the script doesn't exist there, also check:
   grace/rulesbook/codegen/add_connector.sh

2. Open the generated files and verify UCS conventions:
   - Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
   - Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs
   - Registry: crates/integrations/connector-integration/src/connectors.rs (has pub mod {connector_name})

3. Convention checks (fix any violations):
   - Struct is {ConnectorName}<T> (generic), not {ConnectorName}
   - Uses RouterDataV2, not RouterData
   - Uses ConnectorIntegrationV2, not ConnectorIntegration
   - Imports from domain_types, not hyperswitch_domain_models

4. Set up the amount converter:
   macros::create_amount_converter_wrapper!(connector_name: {ConnectorName}, amount_type: {AmountType});

5. Implement ConnectorCommon trait:
   - id() returns "{connector_name}"
   - common_get_content_type() returns "application/json" (or correct type)
   - base_url() returns connectors.{connector_name}.base_url.as_ref()
   - get_auth_header() extracts auth from ConnectorSpecificConfig::{ConnectorName}
   - build_error_response() parses connector error format

6. Add required trait markers:
   - connector_types::ConnectorServiceTrait<T>
   - SourceVerification
   - BodyDecoding

Output:
  STATUS: SUCCESS | FAILED
  FILES_CREATED: [list of files]
  CONVENTION_VIOLATIONS: [none] or [list]
```

---

## Subagent 3: Flow Implementation (per flow)

**Inputs**: connector_name, flow_name, tech_spec_path
**Outputs**: flow implemented

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
8. Reports SUCCESS or FAILED

Output:
  FLOW: {FlowName}
  STATUS: SUCCESS | FAILED
  REASON: (if failed)
```

(End of subagent prompts - Steps 4 & 5 omitted for demo mode)
