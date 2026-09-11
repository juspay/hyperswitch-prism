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
   List them as EXISTING_FLOWS. Cross-check against the macro_connector_flow_status_impls!
   invocation: any marker name still inside its not_implemented: [...] or
   not_supported: [...] list is NOT implemented, whatever trait impls the file appears to
   carry (that macro emits a marker-trait impl for every flow it names).

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
4. FIRST: remove {FlowName}'s marker name from the not_implemented: [...] (or
   not_supported: [...]) list in the macro_connector_flow_status_impls! invocation at the
   bottom of the connector file. That macro (connectors/macros.rs ~:1827) emits BOTH the
   marker-trait impl and a stub ConnectorIntegrationV2 impl for every flow it names, so
   leaving the name there while adding your own is a double E0119.
   Real invocation to copy: connectors/travelhub.rs:455. Skip this step for IncomingWebhook —
   it is a plain trait, not a ConnectorIntegrationV2 flow, and never appears in that macro.
5. Add flow entry to existing create_all_prerequisites! api array
6. Add macro_connector_implementation! block after the existing ones
7. Create request/response types and TryFrom impls in transformers.rs
8. Add the trait marker implementation, now freed by step 4
9. Run: cargo build --package connector-integration
10. Fix any compilation errors

Contract reminders that decide whether this compiles at all:
- Auth comes from req.connector_config (ConnectorSpecificConfig). RouterDataV2 has no
  connector_auth_type field; get_auth_header(&ConnectorAuthType) is E0407. Real signature:
  crates/types-traits/interfaces/src/api.rs:25.
- build_error_response / get_error_response_v2 / get_5xx_error_response each take a third
  parameter, &ConnectorSpecificConfig. The event type is events::Event (there is no
  ConnectorEvent), and it has no set_error_response_body method.
- ConnectorError has exactly five variants, every one carrying a `context` field:
  ResponseDeserializationFailed, ResponseHandlingFailed, UnexpectedResponseError,
  IntegrityCheckFailed, ConnectorErrorResponse. NotImplemented is not one of them — it is an
  IntegrationError variant (a two-element tuple: NotImplemented(String,
  IntegrationErrorContext)). InvalidData and InvalidCard exist on NEITHER enum; read the real
  IntegrationError list in domain_types/src/errors.rs and pick the closest actual variant
  (InvalidDataFormat, InvalidWallet, MismatchedPaymentData, ...), keeping its context field.
- ErrorResponse implements Default: build it with ..Default::default(). attempt_status is
  Option<FlowStatus>, e.g. Some(FlowStatus::Payment(AttemptStatus::Failure)) — and should
  not be forced terminal on the shared error path (connectors/flywire.rs:362-370).
- PaymentsResponseData::TransactionResponse (11 fields) and RefundsResponseData (4 fields)
  are struct/enum-variant literals with no functional-update syntax: every omitted field
  is E0063.
- For IncomingWebhook: get_event_type takes ONE argument besides &self,
  process_payment_webhook takes FOUR, and the error type is WebhookError. There is no
  transformation_status field or WebhookTransformationStatus type.
- SourceVerification and BodyDecoding are non-generic: one impl per connector, never one
  per flow (a per-flow generic impl is E0107).

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
2. Status mapping: no hardcoded statuses outside match arms; the wire status enum carries
   #[serde(other)] Unknown (deserialization layer) and the mapping match has no catch-all
   `_ =>` (mapping layer). Reviewers demand both halves.
3. Code quality: no unwrap(), no None-hardcoded fields, descriptive error messages,
   NO_ERROR_CODE / NO_ERROR_MESSAGE (crates/common/common_utils/src/consts.rs) as the
   error code/message fallbacks rather than .unwrap_or_default(), and an attempt_status on
   the shared error path that is flow-aware rather than a blanket Failure or a blanket None
4. Macro completeness: every new flow in create_all_prerequisites! AND
   macro_connector_implementation!, its marker name REMOVED from
   macro_connector_flow_status_impls!'s not_implemented list, and exactly one trait marker
   impl per flow (a duplicate is E0119)
5. In-band 2xx failures return Err(ErrorResponse { .. }) via a success predicate — see
   utils::is_payment_failure in crates/types-traits/domain_types/src/utils.rs
6. Consistency: new flows follow same patterns as existing flows in this connector
7. Naming: {ConnectorName}{Flow}Request/Response convention
8. Final build: cargo build --package connector-integration

Output:
  CONNECTOR: {ConnectorName}
  VIOLATIONS: [list] or [none]
  STATUS: PASS | FAIL
```
