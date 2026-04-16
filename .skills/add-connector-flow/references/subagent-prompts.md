# Subagent Prompts — add-connector-flow

Each step can be delegated to an independent subagent.

---

## Subagent 1: State Analysis & Dependency Validation

**Inputs**: connector_name, requested_flows
**Outputs**: current state, resolved implementation order, missing prerequisites

```
Analyze the state of the {ConnectorName} connector and validate dependencies for adding
the following flows: {requested_flows}

Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs
Tech spec: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
Dependency reference: .skills/add-connector-flow/references/flow-dependencies.md

Instructions:
1. Verify the connector exists at the expected path. If not → FAILED.

2a. Check tech spec exists at:
    grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
    or: grace/rulesbook/codegen/references/specs/{ConnectorName}.md
    or: grace/rulesbook/codegen/references/specs/{connector_name}.md
    If NONE of these exist → IMMEDIATELY return FAILED. Do NOT continue to steps 3-5.
    Reason: "Tech spec not found. Run generate-tech-spec skill first, or provide the
    tech spec manually. Cannot proceed without a tech spec — do NOT infer API details
    from existing connector code."

2. Read the connector file and identify which flows are already in create_all_prerequisites!
   List them as EXISTING_FLOWS.

3. Read the tech spec for each requested flow's endpoint details.

4. Validate dependencies using the flow-dependencies.md reference:
   - For each requested flow, check that its prerequisites exist in EXISTING_FLOWS
     or are also in the requested set.
   - If a prerequisite is missing → report it and STOP.

5. Determine implementation order (topological sort respecting dependencies).

Output:
  CONNECTOR: {ConnectorName}
  EXISTS: YES | NO
  EXISTING_FLOWS: [Authorize, PSync, Capture, ...]
  REQUESTED_FLOWS: [Refund, RSync, ...]
  MISSING_PREREQUISITES: [none] or [Capture is required for Refund but not implemented]
  IMPLEMENTATION_ORDER: [Refund, RSync]  (dependency-resolved)
  STATUS: READY | BLOCKED
```

---

## Subagent 2: Flow Implementation (per flow)

**Inputs**: connector_name, flow_name, tech_spec_path
**Outputs**: flow implemented, build passes

```
Implement the {FlowName} flow for the existing {ConnectorName} connector.

Tech spec: grace/rulesbook/codegen/references/{connector_name}/technical_specification.md
Implementation guide: .skills/add-connector-flow/references/flow-implementation-guide.md
Flow pattern: .skills/add-connector-flow/references/flow-patterns/{flow}.md
Macro reference: .skills/add-connector-flow/references/macro-reference.md
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Instructions:
1. Read the tech spec for {FlowName} endpoint (URL, method, request/response schema, statuses)
2. Read the flow pattern file for {FlowName}-specific patterns
3. Read the implementation guide for the 3-part procedure
4. Add flow entry to existing create_all_prerequisites! api array
5. Add macro_connector_implementation! block after the existing ones
6. Create request/response types and TryFrom impls in transformers.rs
7. Add the trait marker implementation if not already present
8. Run: cargo build --package connector-integration
9. Fix any compilation errors

Output:
  FLOW: {FlowName}
  STATUS: SUCCESS | FAILED
  BUILD: PASS | FAIL
  FILES_MODIFIED: [list]
  REASON: (if failed)
```

---

## Subagent 3: gRPC Testing

**Inputs**: connector_name, flows_to_test
**Outputs**: test results per flow

```
Test the newly added flows for {ConnectorName} via grpcurl.

Testing guide: .skills/add-connector-flow/references/grpc-testing-guide.md
Credentials: creds.json (field: {connector_name})
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Flows to test: {flow_list}

Instructions:
1. Read the testing guide for grpcurl templates and service/method mapping
2. Start the gRPC server if not running
3. Load credentials from creds.json
4. For each flow, run grpcurl against the correct service/method
5. Validate response against PASS/FAIL criteria in the testing guide
6. If FAILED: read server logs, diagnose, fix code, rebuild, retest (max 7 iterations)
7. Follow anti-loop safeguards (3-strike rule, always change code between retries)

Output:
  CONNECTOR: {ConnectorName}
  RESULTS:
    {FlowName}: PASS | FAIL
    ...
  STATUS: ALL_PASS | PARTIAL | ALL_FAIL
```

---

## Subagent 4: Quality Review

**Inputs**: connector_name
**Outputs**: violations list, pass/fail

```
Quality review the {ConnectorName} connector after adding new flows.

Quality checklist: .skills/add-connector-flow/references/quality-checklist.md
Connector file: crates/integrations/connector-integration/src/connectors/{connector_name}.rs
Transformers: crates/integrations/connector-integration/src/connectors/{connector_name}/transformers.rs

Checks:
1. Architecture: no RouterData (non-V2), no hyperswitch_domain_models imports
2. Status mapping: no hardcoded statuses outside match arms
3. Code quality: no unwrap(), no None-hardcoded fields, descriptive error messages
4. Macro completeness: every flow in both macros + trait markers
5. Consistency: new flows follow same patterns as existing flows in this connector
6. Naming: {ConnectorName}{Flow}Request/Response convention
7. Final build: cargo build --package connector-integration

Output:
  CONNECTOR: {ConnectorName}
  VIOLATIONS: [list] or [none]
  STATUS: PASS | FAIL
```
