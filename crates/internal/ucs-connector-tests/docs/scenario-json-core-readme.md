# Scenario JSON Core Runner (Phase 1)

This document defines the core architecture for scenario-driven UCS tests using one `scenario.json` per suite.

## Goal

Build a common runner where each test is only:

```rust
#[tokio::test]
async fn authorize_no3ds_auto_capture() {
    run_scenario("authorize", "no3ds_auto_capture").await;
}
```

The runner must:

1. Read scenario definition from JSON.
2. Build gRPC request from `grpc_req`.
3. Execute the RPC.
4. Validate response using `assert` rules.

## Scope (for now)

- Include only core scenario engine.
- Use suite-level scenario files.
- Do not include connector overrides.
- Do not include dependency pipeline/composite orchestration yet.

## Directory layout

```text
backend/ucs-connector-tests/
  scenarios/
    authorize/
      scenario.json
    capture/
      scenario.json
    refund/
      scenario.json
    void/
      scenario.json
```

## `scenario.json` shape

Each suite file is a map:

```json
{
  "scenario_name": {
    "grpc_req": { "...": "..." },
    "assert": {
      "response.field.path": { "rule": "..." }
    }
  }
}
```

## Example (`authorize/scenario.json`)

```json
{
  "no3ds_auto_capture": {
    "grpc_req": {
      "amount": { "minor_amount": 6000, "currency": "USD" },
      "auth_type": "NO_THREE_DS",
      "capture_method": "AUTOMATIC",
      "enrolled_for_3ds": false,
      "payment_method": {
        "card": {
          "card_number": "4111111111111111",
          "card_exp_month": "08",
          "card_exp_year": "30",
          "card_cvc": "999",
          "card_holder_name": "joseph Doe"
        }
      }
    },
    "assert": {
      "status": { "one_of": ["CHARGED", "AUTHORIZED", "PENDING"] },
      "connector_transaction_id": { "must_exist": true },
      "error": { "must_not_exist": true }
    }
  },
  "no3ds_manual_capture": {
    "grpc_req": {
      "amount": { "minor_amount": 6000, "currency": "USD" },
      "auth_type": "NO_THREE_DS",
      "capture_method": "MANUAL",
      "enrolled_for_3ds": false,
      "payment_method": {
        "card": {
          "card_number": "4111111111111111",
          "card_exp_month": "08",
          "card_exp_year": "30",
          "card_cvc": "999",
          "card_holder_name": "joseph Doe"
        }
      }
    },
    "assert": {
      "status": { "one_of": ["AUTHORIZED"] },
      "connector_transaction_id": { "must_exist": true },
      "error": { "must_not_exist": true }
    }
  },
  "no3ds_fail_payment": {
    "grpc_req": {
      "amount": { "minor_amount": 6000, "currency": "USD" },
      "auth_type": "NO_THREE_DS",
      "capture_method": "AUTOMATIC",
      "enrolled_for_3ds": false,
      "payment_method": {
        "card": {
          "card_number": "4000000000000002",
          "card_exp_month": "01",
          "card_exp_year": "35",
          "card_cvc": "123",
          "card_holder_name": "joseph Doe"
        }
      }
    },
    "assert": {
      "status": { "one_of": ["FAILURE", "AUTHORIZATION_FAILED", "ROUTER_DECLINED"] },
      "connector_transaction_id": { "must_not_exist": true },
      "error": { "must_exist": true },
      "error.connector_details.message": { "contains": "declin" }
    }
  }
}
```

## Assertion DSL (core)

Supported rules for `assert` values:

- `{ "must_exist": true }`
- `{ "must_not_exist": true }`
- `{ "equals": <json_value> }`
- `{ "one_of": [<value1>, <value2>] }`
- `{ "contains": "substring" }`
- `{ "echo": "request.field.path" }`

`echo` means compare a response field with a field from the request payload.

## Runtime flow

1. `load_scenario(suite, scenario_name)` reads JSON entry.
2. `call_grpc(suite, grpc_req_json)` executes the corresponding RPC:
   - `authorize` -> `PaymentService::authorize`
   - `capture` -> `PaymentService::capture`
   - `refund` -> `PaymentService::refund`
   - `void` -> `PaymentService::void`
3. Convert response to JSON.
4. `assert_response(assert_rules, response_json, grpc_req_json)` validates all rules.

## Minimal API contract

```rust
pub async fn run_scenario(suite: &str, scenario_name: &str) {
    let scenario = load_scenario(suite, scenario_name);
    let response_json = call_grpc(suite, &scenario.grpc_req).await;
    assert_response(&scenario.assert_rules, &response_json, &scenario.grpc_req);
}
```

## Implementation plan

### Milestone 1: Schema and loader

- Add data structs:
  - `ScenarioFile`
  - `ScenarioDef { grpc_req, assert }`
  - `FieldAssert` enum
- Implement loader from `scenarios/<suite>/scenario.json`.

### Milestone 2: Assertion engine

- Implement field-path lookup (`a.b.c`).
- Implement rule evaluators for all DSL operators.
- Add clear assertion failure messages with field path.

### Milestone 3: gRPC caller

- Implement suite-based dispatch (`match suite`).
- Convert request JSON -> typed proto request.
- Execute RPC.
- Convert typed proto response -> JSON.

### Milestone 4: common runner + first tests

- Implement `run_scenario(suite, scenario_name)`.
- Add thin tests for authorize scenarios that only call runner.

### Milestone 5: reporting

- Emit run result JSON (scenario, suite, pass/fail, error message).
- Add markdown summary generation.

## Out of scope (future phases)

- Connector-level overrides and connector-specific request patches.
- Dependency graph execution (`authorize -> capture -> refund`).
- Composite pipelines and prereq data sharing.
- Auto-generation from grpcurl command text.

## Definition of done (core phase)

Core phase is done when:

1. A scenario in `authorize/scenario.json` can be executed end-to-end.
2. Assertions are fully data-driven from JSON.
3. A test function only calls `run_scenario(...)`.
4. Result report JSON and markdown are produced.
