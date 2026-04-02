# Documentation Generator Architecture Refactor

## Problem Statement

The documentation generator produces runnable SDK examples in 4 languages (Python, JavaScript, Kotlin, Rust) for each connector. The output quality is inconsistent across languages because **each language renderer independently re-implements behavioral decisions**.

### Concrete Inconsistencies

1. **Different scenario counts** — JS generates 2 scenarios for a connector while Rust generates 4, because `_scenario_step_javascript` and `_scenario_step_rust` have different `if flow_key ==` branches.

2. **Different status handling** — Given the same 200 response:
   - JS: authorize returns success
   - Kotlin: authorize returns 200 but surfaces `status=FAILED`
   
   Because Kotlin checks `.status.name` while JS checks `.status ===`, and the status-to-enum mapping differs.

3. **Different error behavior** — Given a 400 response:
   - JS: `throw new Error(...)` for capture/refund/recurring_charge
   - Python: `raise RuntimeError(...)` for the same set
   - Rust: also checks `refund` separately with `RefundStatus::RefundFailure` (different enum)
   - Rust: checks `status_code >= 400` for authentication flows (no other language does this)

4. **Different return shapes** — `_scenario_return_python()`, `_scenario_return_javascript()`, `_scenario_return_kotlin()`, `_scenario_return_rust()` are four independent `if/elif` chains (lines 873-893, 1069-1088, 1147-1166, 1245-1272) that each hardcode which fields to return per scenario.

### Root Cause

The root cause is NOT globals, file size, or missing OOP patterns. It's that **there is no shared behavioral specification**. Four functions independently decide:

| Decision | Where it's made | Times duplicated |
|----------|-----------------|------------------|
| Which flows get status checks | `_scenario_step_{lang}` | 4x (one per language) |
| Which status values to check | `_scenario_step_{lang}` | 4x |
| Whether to raise/throw/return on error | `_scenario_step_{lang}` | 4x |
| Which response fields to return | `_scenario_return_{lang}` | 4x |
| Cross-flow field references | `_DYNAMIC_FIELDS_{lang}` | 5x (Python, JS, Kotlin, Rust struct, Rust JSON) |

Any time someone adds a new scenario or modifies status handling, they must update all 4-5 copies identically. They don't. That's the bug.

### Multi-Step Flow Linking Is Undocumented

Connecting flows into multi-step sequences (authorize → capture → refund) is a nightmare. There is **no machine-readable source of truth** for which flows depend on which, what fields flow between them, or how this varies per connector.

#### Where flow dependencies currently live (3 disconnected places):

**1. Generator hardcode** — `_DYNAMIC_FIELDS` in `snippet_examples/generate.py` (5 copies):
```python
# This is the ONLY place that says "capture needs connector_transaction_id from authorize"
_DYNAMIC_FIELDS = {
    ("checkout_card", "capture", "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ("refund",        "refund",  "connector_transaction_id"): "authorize_response.connector_transaction_id",
    ...
}
# Plus _DYNAMIC_FIELDS_JS, _DYNAMIC_FIELDS_KT, _DYNAMIC_FIELDS_RS, _DYNAMIC_FIELDS_RS_JSON
```

**2. Test suite specs** — `global_suites/*/suite_spec.json`:
```json
{
  "suite": "capture",
  "depends_on": [{"suite": "authorize", "context_map": {}}],
}
{
  "suite": "recurring_charge",
  "depends_on": [{"suite": "setup_recurring", "context_map": {
    "connector_recurring_payment_id...connector_mandate_id": "res.mandate_reference...connector_mandate_id"
  }}]
}
```

**3. Rust router** — `scenario_api.rs` lists "context deferred paths" that are expected to come from prior flows:
```rust
fn context_deferred_paths() -> Vec<String> {
    vec![
        "connector_transaction_id.id",
        "customer.connector_customer_id",
        "state.access_token.token.value",
        "refund_id",
    ]
}
```

**None of these reference each other.** The test suite says "capture depends on authorize" but doesn't say which field. The generator says "capture.connector_transaction_id comes from authorize" but only for specific scenarios. The Rust router has a flat list of deferred paths with no flow context.

#### Connector-specific variations are real but invisible

Different connectors have fundamentally different flow topologies:

| Pattern | Connectors | Flow Chain |
|---------|-----------|------------|
| Standard | stripe, adyen, checkout, ... (18) | authorize → capture/refund/void/get |
| Pre-auth required | nexixpay, redsys | pre_authenticate → capture/refund/void |
| Access token first | paypal, airwallex, globalpay, jpmorgan | create_access_token → authorize → ... |
| Order creation first | razorpay, airwallex, trustpay | create_order → authorize → ... |
| Customer creation first | stripe, finix, authorizedotnet | create_customer → authorize → ... |
| Tokenize-then-pay | stripe, finix, billwerk, hipay | tokenize → authorize (with token) |
| No capture (auto-only) | mollie, multisafepay, nexinets | authorize → refund/void (no separate capture) |
| No authorize | nexixpay, redsys, payload, payme | pre_authenticate/create_order → capture directly |
| Get-only | calida, cryptopay, loonio, mifinity | get (no payment initiation) |

Today, the generator treats all connectors as if they follow the "Standard" pattern. If a connector needs `create_access_token` before `authorize`, the generator doesn't know — and the generated example doesn't include that prerequisite step. The developer copies the example, runs it, and gets an auth error with no explanation.

### Secondary Issues

These are real but less impactful than the behavior divergence and flow linking problems:

1. **Global state** — `_SCENARIO_GROUPS` populated via `set_scenario_groups()`
2. **Hardcoded configuration** — `_DISPLAY_NAMES`, `_PROBE_PM_BY_CATEGORY` in Python code
3. **File size** — `snippet_examples/generate.py` is 3,458 lines

## Goals

1. **Behavioral consistency** — All languages implement identical status checks, error handling, and return shapes
2. **Single source of truth** — One spec defines behavior; renderers translate mechanically
3. **Drift detection** — Structural checks catch divergence before it ships
4. **Flow graph** — Machine-readable, connector-specific flow dependencies with field-level linking
5. **Field-probe preserved** — Probe data remains pure request payloads
6. **Idiomatic output preserved** — Each language still uses native patterns (Kotlin builders, Rust match, etc.)

## Core Design

### 1. Behavioral Spec (The Key Change)

The scenario YAML defines not just which flows to run, but **how to handle every response state**. This is the single source of truth that all renderers must implement.

```yaml
# specs/scenarios.yaml
scenarios:
  - key: "checkout_card"
    name: "Card Payment (Authorize + Capture)"
    description: "Two-step card payment. First authorize, then capture."
    flows:
      - name: "authorize"
        pm_type: "Card"
        required: true
        capture_method: "MANUAL"
        status_handling:
          - status: ["FAILED", "AUTHORIZATION_FAILED"]
            action: "error"
            message: "Payment failed: {error}"
          - status: ["PENDING"]
            action: "return_early"
            return_fields:
              status: "pending"
              transaction_id: "{response.connector_transaction_id}"

      - name: "capture"
        pm_type: null  # Uses "default" key in probe
        required: false
        depends_on: "authorize"
        use_from_previous: ["connector_transaction_id"]
        status_handling:
          - status: ["FAILED"]
            action: "error"
            message: "Capture failed: {error}"

    return_fields:
      status: "{capture_response.status}"
      transaction_id: "{authorize_response.connector_transaction_id}"
      error: "{capture_response.error}"

  - key: "checkout_autocapture"
    name: "One-step Payment (Authorize + Capture)"
    description: "Simple payment that authorizes and captures in one call."
    flows:
      - name: "authorize"
        pm_type: null  # Try variants: Card, Ach, Sepa, GooglePay, ApplePay
        pm_type_variants: ["Card", "Ach", "Sepa", "Bacs", "GooglePay", "ApplePay"]
        required: true
        capture_method: "AUTOMATIC"
        status_handling:
          - status: ["FAILED", "AUTHORIZATION_FAILED"]
            action: "error"
            message: "Payment failed: {error}"
          - status: ["PENDING"]
            action: "return_early"
            return_fields:
              status: "pending"
              transaction_id: "{response.connector_transaction_id}"

    return_fields:
      status: "{authorize_response.status}"
      transaction_id: "{authorize_response.connector_transaction_id}"
      error: "{authorize_response.error}"

  - key: "refund"
    name: "Refund"
    description: "Return funds to the customer for a completed payment."
    flows:
      - name: "authorize"
        pm_type: null
        pm_type_variants: ["Card", "Ach", "Sepa", "Bacs", "GooglePay", "ApplePay"]
        required: true
        capture_method: "AUTOMATIC"
        status_handling:
          - status: ["FAILED", "AUTHORIZATION_FAILED"]
            action: "error"
            message: "Payment failed: {error}"
          - status: ["PENDING"]
            action: "return_early"
            return_fields:
              status: "pending"
              transaction_id: "{response.connector_transaction_id}"

      - name: "refund"
        pm_type: null
        required: true
        depends_on: "authorize"
        use_from_previous: ["connector_transaction_id"]
        status_handling:
          - status: ["FAILED", "REFUND_FAILURE"]
            action: "error"
            message: "Refund failed: {error}"

    return_fields:
      status: "{refund_response.status}"
      error: "{refund_response.error}"
```

**Key principle**: `status_handling` and `return_fields` are defined ONCE. All 4 languages implement them. No independent `if/elif` chains.

### 2. Status Mapping Table (Per-Language)

Each language maps abstract status strings to its native representation:

```yaml
# config/status_mapping.yaml
python:
  status_access: '{var}.status'
  status_compare: '"{status}"'
  error_access: '{var}.error'
  error_statement: 'raise RuntimeError(f"{message}")'
  return_statement: 'return {fields}'
  field_access: '{var}.{field}'

javascript:
  status_access: '{var}.status'
  status_compare: "'{status}'"
  error_access: '{var}.error?.message'
  error_statement: 'throw new Error(`{message}`)'
  return_statement: 'return {fields}'
  field_access: '{var}.{camel_field}'

kotlin:
  status_access: '{var}.status.name'
  status_compare: '"{status}"'
  error_access: '{var}.error.unifiedDetails.message'
  error_statement: 'throw RuntimeException("{message}")'
  return_statement: 'return mapOf({fields})'
  field_access: '{var}.{camel_field}'

rust:
  status_access: '{var}.status()'
  # Rust uses enum variants, not string comparison
  status_enum_map:
    FAILED: 'PaymentStatus::Failure'
    AUTHORIZATION_FAILED: 'PaymentStatus::AuthorizationFailed'
    PENDING: 'PaymentStatus::Pending'
    REFUND_FAILURE: 'RefundStatus::RefundFailure'
  error_access: '{var}.error'
  error_statement: 'return Err(format!("{message}").into())'
  return_statement: 'Ok(format!("{format_string}", {fields}))'
  field_access: '{var}.{field}.as_deref().unwrap_or("")'
```

This replaces the hardcoded status checks scattered across four `_scenario_step_*` functions.

### 3. Connector Flow Graph (The Missing Piece)

The system currently has no machine-readable source of truth for:
- Which flows a connector supports
- What order they must be called in
- What fields flow from one step to the next
- What prerequisite flows exist (create_access_token, create_customer, create_order)

#### The flow_graph: generated by field-probe, per connector

Field-probe already runs against each connector and knows which flows are supported. We extend it to also emit a `flow_graph` section in each connector's probe JSON:

```json
// data/field_probe/stripe.json (new section)
{
  "connector": "stripe",
  "flow_graph": {
    "nodes": {
      "create_customer": {
        "type": "prerequisite",
        "description": "Create a customer profile for tokenization and recurring payments",
        "provides": {
          "connector_customer_id": {
            "response_path": "connector_customer_id",
            "description": "Stripe customer ID (cus_xxx)"
          }
        }
      },
      "authorize": {
        "type": "entry_point",
        "description": "Authorize a payment, reserving funds",
        "provides": {
          "connector_transaction_id": {
            "response_path": "connector_transaction_id",
            "description": "Stripe PaymentIntent ID (pi_xxx)"
          }
        }
      },
      "capture": {
        "type": "dependent",
        "description": "Capture previously authorized funds",
        "requires": {
          "connector_transaction_id": {
            "from_flow": "authorize",
            "from_field": "connector_transaction_id",
            "request_path": "connector_transaction_id"
          }
        }
      },
      "refund": {
        "type": "dependent",
        "description": "Refund a completed payment",
        "requires": {
          "connector_transaction_id": {
            "from_flow": "authorize",
            "from_field": "connector_transaction_id",
            "request_path": "connector_transaction_id"
          }
        }
      },
      "void": {
        "type": "dependent",
        "description": "Cancel an uncaptured authorization",
        "requires": {
          "connector_transaction_id": {
            "from_flow": "authorize",
            "from_field": "connector_transaction_id",
            "request_path": "connector_transaction_id"
          }
        }
      },
      "setup_recurring": {
        "type": "entry_point",
        "description": "Set up a mandate for recurring charges",
        "provides": {
          "mandate_reference": {
            "response_path": "mandate_reference.connector_mandate_id.connector_mandate_id",
            "description": "Stripe mandate/PaymentMethod ID for recurring"
          }
        }
      },
      "recurring_charge": {
        "type": "dependent",
        "description": "Charge against an existing mandate",
        "requires": {
          "connector_recurring_payment_id": {
            "from_flow": "setup_recurring",
            "from_field": "mandate_reference",
            "request_path": "connector_recurring_payment_id.connector_mandate_id.connector_mandate_id"
          },
          "connector_customer_id": {
            "from_flow": "create_customer",
            "from_field": "connector_customer_id",
            "request_path": "connector_customer_id"
          }
        }
      },
      "tokenize": {
        "type": "entry_point",
        "description": "Tokenize a payment method for later use",
        "provides": {
          "payment_method_token": {
            "response_path": "payment_method_token",
            "description": "Stripe PaymentMethod ID (pm_xxx)"
          }
        }
      }
    },
    "edges": [
      {"from": "authorize",        "to": "capture"},
      {"from": "authorize",        "to": "refund"},
      {"from": "authorize",        "to": "void"},
      {"from": "authorize",        "to": "get"},
      {"from": "setup_recurring",  "to": "recurring_charge"},
      {"from": "create_customer",  "to": "recurring_charge"},
      {"from": "create_customer",  "to": "setup_recurring"}
    ]
  },
  "flows": { ... }  // existing probe data unchanged
}
```

Compare with a connector that has a different topology:

```json
// data/field_probe/paypal.json — needs access token before everything
{
  "connector": "paypal",
  "flow_graph": {
    "nodes": {
      "create_access_token": {
        "type": "prerequisite",
        "description": "Obtain OAuth2 access token for PayPal API",
        "provides": {
          "access_token": {
            "response_path": "state.access_token.token.value",
            "description": "Bearer token for subsequent API calls"
          }
        }
      },
      "authorize": {
        "type": "entry_point",
        "description": "Create PayPal order and authorize payment",
        "requires": {
          "access_token": {
            "from_flow": "create_access_token",
            "from_field": "access_token",
            "request_path": "state.access_token.token.value"
          }
        },
        "provides": {
          "connector_transaction_id": {
            "response_path": "connector_transaction_id",
            "description": "PayPal order ID"
          }
        }
      },
      "capture": {
        "type": "dependent",
        "requires": {
          "access_token": {
            "from_flow": "create_access_token",
            "from_field": "access_token",
            "request_path": "state.access_token.token.value"
          },
          "connector_transaction_id": {
            "from_flow": "authorize",
            "from_field": "connector_transaction_id",
            "request_path": "connector_transaction_id"
          }
        }
      }
    },
    "edges": [
      {"from": "create_access_token", "to": "authorize"},
      {"from": "create_access_token", "to": "capture"},
      {"from": "create_access_token", "to": "refund"},
      {"from": "authorize",           "to": "capture"},
      {"from": "authorize",           "to": "refund"},
      {"from": "authorize",           "to": "void"},
      {"from": "authorize",           "to": "get"}
    ]
  }
}
```

```json
// data/field_probe/razorpay.json — needs order creation before authorize
{
  "connector": "razorpay",
  "flow_graph": {
    "nodes": {
      "create_order": {
        "type": "prerequisite",
        "description": "Create a Razorpay order before payment authorization",
        "provides": {
          "order_id": {
            "response_path": "connector_order_id",
            "description": "Razorpay order ID (order_xxx)"
          }
        }
      },
      "authorize": {
        "type": "entry_point",
        "requires": {
          "order_id": {
            "from_flow": "create_order",
            "from_field": "order_id",
            "request_path": "merchant_order_id"
          }
        },
        "provides": {
          "connector_transaction_id": {
            "response_path": "connector_transaction_id",
            "description": "Razorpay payment ID (pay_xxx)"
          }
        }
      }
    },
    "edges": [
      {"from": "create_order", "to": "authorize"},
      {"from": "authorize",    "to": "capture"},
      {"from": "authorize",    "to": "refund"}
    ]
  }
}
```

#### Node types

| Type | Meaning | Example |
|------|---------|---------|
| `prerequisite` | Must run before entry points, provides tokens/IDs/sessions | create_access_token, create_customer, create_order |
| `entry_point` | Starts a payment lifecycle, can be called directly | authorize, setup_recurring, tokenize, pre_authenticate |
| `dependent` | Requires output from a prior flow | capture, refund, void, get, recurring_charge |

#### Every dependency is explicit

The SDK is a thin gRPC client — it calls exactly one API at a time and does nothing extra. There is no magic. If `authorize` requires an access token, the developer must call `create_access_token` first and pass the result. The generated examples must show every step.

```json
"requires": {
  "access_token": {
    "from_flow": "create_access_token",
    "from_field": "access_token",
    "request_path": "state.access_token.token.value"
  },
  "connector_transaction_id": {
    "from_flow": "authorize",
    "from_field": "connector_transaction_id",
    "request_path": "connector_transaction_id"
  }
}
```

Every entry in `requires` means: the generated example MUST include the source flow as a prior step, and MUST show the developer how to pass the field from one response to the next request.

#### How field-probe generates the flow graph

Field-probe already knows:
1. Which flows are supported per connector (it runs them)
2. The proto request/response schemas (from `manifest.json`)
3. Which fields are "context deferred" (from `scenario_api.rs`)

The generation logic:

```python
def build_flow_graph(connector_name: str, probe_data: dict, manifest: dict) -> dict:
    """
    Build flow_graph from probe data + proto metadata.
    
    1. For each supported flow, create a node
    2. Examine request fields — if a field matches a known "provides" 
       from another flow, create a requires entry
    3. Build edges from requires relationships
    """
    nodes = {}
    
    # Known provider fields (derived from proto response types)
    KNOWN_PROVIDERS = {
        "connector_transaction_id": ("authorize", "connector_transaction_id"),
        "connector_customer_id":    ("create_customer", "connector_customer_id"),
        "connector_order_id":       ("create_order", "connector_order_id"),
        "access_token":             ("create_access_token", "state.access_token.token.value"),
        "mandate_reference":        ("setup_recurring", "mandate_reference.connector_mandate_id.connector_mandate_id"),
        "payment_method_token":     ("tokenize", "payment_method_token"),
        "refund_id":                ("refund", "connector_refund_id"),
    }
    
    for flow_key, pm_map in probe_data.get("flows", {}).items():
        # Determine what this flow provides (from response schema)
        provides = detect_provides(flow_key, manifest)
        
        # Determine what this flow requires (from request fields that match known providers)
        requires = {}
        for pm_key, pm_data in pm_map.items():
            if pm_data.get("status") != "supported":
                continue
            request = pm_data.get("proto_request", {})
            requires = detect_requires(request, KNOWN_PROVIDERS, connector_name)
            break  # Only need one PM's request to detect structure
        
        # Classify node type
        if requires and any(r["from_flow"] in ("authorize", "setup_recurring") for r in requires.values()):
            node_type = "dependent"
        elif flow_key in ("create_access_token", "create_customer", "create_order"):
            node_type = "prerequisite"
        else:
            node_type = "entry_point"
        
        nodes[flow_key] = {
            "type": node_type,
            "provides": provides,
            "requires": requires,
        }
    
    # Build edges from requires
    edges = []
    for flow_key, node in nodes.items():
        for req_info in node.get("requires", {}).values():
            edges.append({"from": req_info["from_flow"], "to": flow_key})
    
    return {"nodes": nodes, "edges": dedupe(edges)}
```

#### How the hydrator uses the flow graph

The hydrator resolves scenarios against the connector's flow graph:

```python
# core/hydrator.py (updated)

class ScenarioHydrator:
    def __init__(self, probe_data: dict, connector_name: str):
        self.probe = probe_data
        self.connector_name = connector_name
        self.flow_graph = probe_data.get("flow_graph", {})
    
    def hydrate(self, scenario: Scenario) -> Optional[HydratedScenario]:
        # ... existing flow resolution logic ...
        
        # NEW: Prepend prerequisite flows from flow_graph
        prerequisite_flows = self._resolve_prerequisites(scenario)
        
        # NEW: Resolve field links from flow_graph instead of hardcoded _DYNAMIC_FIELDS
        for flow in hydrated_flows:
            flow.field_links = self._resolve_field_links(flow.name)
        
        return HydratedScenario(
            # ...
            prerequisite_flows=prerequisite_flows,
            flows=hydrated_flows,
        )
    
    def _resolve_prerequisites(self, scenario: Scenario) -> list[HydratedFlow]:
        """
        Walk the flow_graph to find prerequisite flows needed by this scenario.
        
        Example: For checkout_card (authorize → capture) on paypal,
        discovers that authorize requires create_access_token.
        Returns [HydratedFlow(name="create_access_token", ...)]
        """
        nodes = self.flow_graph.get("nodes", {})
        needed = set()
        
        for flow_def in scenario.flows:
            node = nodes.get(flow_def.name, {})
            for req_key, req_info in node.get("requires", {}).items():
                source_flow = req_info.get("from_flow")
                source_node = nodes.get(source_flow, {})
                if source_node.get("type") == "prerequisite":
                    needed.add(source_flow)
        
        # Hydrate each prerequisite
        prereqs = []
        for flow_name in sorted(needed):  # Deterministic order
            flow_data = self._resolve_flow_data_by_name(flow_name)
            if flow_data and flow_data.get("status") == "supported":
                prereqs.append(HydratedFlow(
                    name=flow_name,
                    payload=flow_data.get("proto_request", {}),
                    status_handling=[],  # Prerequisites don't need status handling in examples
                    is_prerequisite=True,
                ))
        
        return prereqs
    
    def _resolve_field_links(self, flow_name: str) -> dict[str, FieldLink]:
        """
        Get field-level dependencies for a flow from the flow_graph.
        
        Returns: {"connector_transaction_id": FieldLink(from_flow="authorize", ...)}
        
        Every dependency is explicit — the SDK does not auto-inject anything.
        All links produce generated code showing the developer how to pass
        the field from one response to the next request.
        
        This replaces the 5 hardcoded _DYNAMIC_FIELDS dicts.
        """
        node = self.flow_graph.get("nodes", {}).get(flow_name, {})
        links = {}
        for field_key, req_info in node.get("requires", {}).items():
            links[req_info["request_path"]] = FieldLink(
                from_flow=req_info["from_flow"],
                from_field=req_info["from_field"],
                target_path=req_info["request_path"],
            )
        return links
```

#### What the generated example looks like AFTER this change

**Before (current)** — PayPal checkout_card example:
```python
# Missing create_access_token! Developer will get auth errors.
async def process_checkout_card():
    authorize_response = await client.authorize(build_authorize_request("MANUAL"))
    capture_response = await client.capture(build_capture_request(authorize_response.connector_transaction_id))
```

**After** — PayPal checkout_card example:
```python
async def process_checkout_card():
    # Step 1: Create Access Token — PayPal requires an OAuth2 token before any API call
    token_response = await client.create_access_token(build_create_access_token_request())

    # Step 2: Authorize — create PayPal order and authorize payment
    authorize_response = await client.authorize(build_authorize_request(
        "MANUAL",
        access_token=token_response.access_token  # Bearer token from Step 1
    ))
    if authorize_response.status == "FAILED":
        raise RuntimeError(f"Payment failed: {authorize_response.error}")

    # Step 3: Capture — capture the authorized PayPal order
    capture_response = await client.capture(build_capture_request(
        connector_transaction_id=authorize_response.connector_transaction_id,  # PayPal order ID from Step 2
        access_token=token_response.access_token  # Same token needed for every call
    ))
```

**Before** — Razorpay checkout_card example:
```python
# Missing create_order! Razorpay requires an order before authorize.
async def process_checkout_card():
    authorize_response = await client.authorize(build_authorize_request("MANUAL"))
    ...
```

**After** — Razorpay checkout_card example:
```python
async def process_checkout_card():
    # Step 1: Create Order — Razorpay requires an order before payment authorization
    order_response = await client.create_order(build_create_order_request())

    # Step 2: Authorize — authorize payment against the Razorpay order
    authorize_response = await client.authorize(build_authorize_request(
        "MANUAL",
        merchant_order_id=order_response.connector_order_id  # Razorpay order ID from Step 1
    ))
    ...
```

### 4. Hydration Layer

Combines abstract scenarios with field-probe data. Handles missing flows gracefully.

```python
# core/hydrator.py
from typing import Optional

class ScenarioHydrator:
    def __init__(self, probe_data: dict, connector_name: str):
        self.probe = probe_data
        self.connector_name = connector_name

    def hydrate(self, scenario: Scenario) -> Optional[HydratedScenario]:
        """
        Fill scenario with probe payloads.

        Returns None if required flows are missing (scenario not supported).
        """
        hydrated_flows = []
        skipped_optional = []

        for flow in scenario.flows:
            flow_data = self._resolve_flow_data(flow)

            if not flow_data:
                if flow.required:
                    return None  # Required flow missing — skip entire scenario
                skipped_optional.append(flow.name)
                continue

            if flow_data.get("status") != "supported":
                if flow.required:
                    return None
                skipped_optional.append(flow.name)
                continue

            hydrated_flows.append(HydratedFlow(
                name=flow.name,
                payload=flow_data.get("proto_request", {}),
                status_handling=flow.status_handling,
                depends_on=flow.depends_on,
                use_from_previous=flow.use_from_previous,
                capture_method=flow.capture_method,
            ))

        return HydratedScenario(
            key=scenario.key,
            name=scenario.name,
            description=scenario.description,
            connector_name=self.connector_name,
            flows=hydrated_flows,
            return_fields=scenario.return_fields,
            skipped_optional=skipped_optional,
        )

    def _resolve_flow_data(self, flow: FlowDefinition) -> Optional[dict]:
        """Resolve flow data from probe, trying PM variants if specified."""
        flows = self.probe.get("flows", {})
        flow_entry = flows.get(flow.name, {})

        if flow.pm_type:
            return flow_entry.get(flow.pm_type)

        if flow.pm_type_variants:
            for variant in flow.pm_type_variants:
                data = flow_entry.get(variant)
                if data and data.get("status") == "supported" and data.get("proto_request"):
                    return data
            return None

        return flow_entry.get("default")
```

### 5. Renderer Contract

Each renderer receives a `HydratedScenario` and translates its `status_handling` rules and `return_fields` mechanically. The renderer does NOT decide what to check — it only decides HOW to express it.

```python
# renderers/base.py
from abc import ABC, abstractmethod

class BaseRenderer(ABC):
    def __init__(self, hydrated_scenario: HydratedScenario, status_mapping: dict):
        self.scenario = hydrated_scenario
        self.status_map = status_mapping
        self.flow_vars: dict[str, str] = {}  # flow_name -> response variable

    def render(self) -> str:
        """Render the complete scenario. Subclasses override for language structure."""
        lines = self._render_function_header()

        for flow in self.scenario.flows:
            var = self._assign_response_var(flow)
            self.flow_vars[flow.name] = var

            # 1. Build and execute the request
            payload = self._substitute_dependencies(flow.payload, flow)
            lines.extend(self._render_request(flow, payload, var))

            # 2. Status handling — from spec, NOT hardcoded
            for rule in flow.status_handling:
                lines.extend(self._render_status_rule(var, rule))

        # 3. Return — from spec, NOT hardcoded
        lines.extend(self._render_return(self.scenario.return_fields))

        lines.extend(self._render_function_footer())
        return "\n".join(lines)

    def _substitute_dependencies(self, payload: dict, flow: HydratedFlow) -> dict:
        """Replace use_from_previous fields with references to prior responses."""
        import copy
        result = copy.deepcopy(payload)

        if not flow.use_from_previous or not flow.depends_on:
            return result

        prev_var = self.flow_vars.get(flow.depends_on, f"{flow.depends_on}_response")
        for field in flow.use_from_previous:
            if field in result:
                result[field] = self._make_field_reference(prev_var, field)

        return result

    # ── Abstract methods — each language implements these ──

    @abstractmethod
    def _render_function_header(self) -> list[str]:
        """Language-specific function signature and setup."""
        pass

    @abstractmethod
    def _render_function_footer(self) -> list[str]:
        """Language-specific function closing."""
        pass

    @abstractmethod
    def _render_request(self, flow: HydratedFlow, payload: dict, var: str) -> list[str]:
        """Render the API call with payload serialization."""
        pass

    @abstractmethod
    def _render_status_rule(self, response_var: str, rule: StatusRule) -> list[str]:
        """
        Translate ONE status rule into language-specific code.
        
        Given: StatusRule(status=["FAILED"], action="error", message="Payment failed: {error}")
        Python emits:  if var.status == "FAILED": raise RuntimeError(...)
        JS emits:      if (var.status === 'FAILED') { throw new Error(...) }
        Kotlin emits:  "FAILED" -> throw RuntimeException(...)
        Rust emits:    PaymentStatus::Failure => return Err(...)
        """
        pass

    @abstractmethod
    def _render_return(self, return_fields: dict[str, str]) -> list[str]:
        """
        Translate return_fields spec into language-specific return statement.
        
        Given: {status: "{capture_response.status}", transaction_id: "{authorize_response.connector_transaction_id}"}
        Python emits:  return {"status": capture_response.status, ...}
        JS emits:      return { status: captureResponse.status, ... }
        Kotlin emits:  return mapOf("status" to captureResponse.status.name, ...)
        Rust emits:    Ok(format!("Payment: {:?}", ...))
        """
        pass

    @abstractmethod
    def _assign_response_var(self, flow: HydratedFlow) -> str:
        """Assign a response variable name (language-specific naming convention)."""
        pass

    @abstractmethod
    def _make_field_reference(self, var_name: str, field: str) -> str:
        """
        Create a field reference marker for payload substitution.
        
        Must handle language-specific patterns:
        - Python:  authorize_response.connector_transaction_id
        - JS:      authorizeResponse.connectorTransactionId
        - Kotlin:  authorizeResponse.connectorTransactionId (inside builder)
        - Rust:    &authorize_response.connector_transaction_id
        """
        pass


# renderers/python_renderer.py
class PythonRenderer(BaseRenderer):

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> list[str]:
        lines = []
        for status in rule.status:
            lines.append(f'    if {response_var}.status == "{status}":')
            if rule.action == "error":
                msg = rule.message.replace("{error}", f'{{{response_var}.error}}')
                lines.append(f'        raise RuntimeError(f"{msg}")')
            elif rule.action == "return_early":
                fields = self._format_return_fields(rule.return_fields, response_var)
                lines.append(f'        return {fields}')
        lines.append("")
        return lines

    def _render_return(self, return_fields: dict[str, str]) -> list[str]:
        parts = []
        for key, ref in return_fields.items():
            resolved = self._resolve_field_ref(ref)
            parts.append(f'"{key}": {resolved}')
        return [f'    return {{{", ".join(parts)}}}']

    def _make_field_reference(self, var_name: str, field: str) -> str:
        return f"__REF__:{var_name}.{field}"

    def _assign_response_var(self, flow: HydratedFlow) -> str:
        return f"{flow.name}_response"

    # ... _render_request, _render_function_header, etc.


# renderers/javascript_renderer.py
class JavaScriptRenderer(BaseRenderer):

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> list[str]:
        lines = []
        for status in rule.status:
            lines.append(f"    if ({response_var}.status === '{status}') {{")
            if rule.action == "error":
                msg = rule.message.replace("{error}", f'${{{response_var}.error?.message}}')
                lines.append(f'        throw new Error(`{msg}`);')
            elif rule.action == "return_early":
                fields = self._format_return_fields(rule.return_fields, response_var)
                lines.append(f'        return {fields};')
            lines.append("    }")
        lines.append("")
        return lines

    def _assign_response_var(self, flow: HydratedFlow) -> str:
        return _to_camel(f"{flow.name}_response")

    def _make_field_reference(self, var_name: str, field: str) -> str:
        return f"__REF__:{var_name}.{_to_camel(field)}"

    # ...


# renderers/rust_renderer.py
class RustRenderer(BaseRenderer):

    def _render_status_rule(self, response_var: str, rule: StatusRule) -> list[str]:
        """Rust uses match blocks and enum variants instead of string comparison."""
        lines = []
        # Group status values into a single match arm
        enum_variants = [self.status_map["status_enum_map"].get(s, s) for s in rule.status]
        pattern = " | ".join(enum_variants)

        if rule.action == "error":
            msg = rule.message.replace("{error}", f'{{:?}}", {response_var}.error')
            lines.append(f'    if matches!({response_var}.status(), {pattern}) {{')
            lines.append(f'        return Err(format!("{msg}).into());')
            lines.append(f'    }}')
        elif rule.action == "return_early":
            lines.append(f'    if matches!({response_var}.status(), {pattern}) {{')
            lines.append(f'        return Ok("pending — awaiting webhook".to_string());')
            lines.append(f'    }}')

        lines.append("")
        return lines

    def _make_field_reference(self, var_name: str, field: str) -> str:
        return f"__REF__:&{var_name}.{field}"

    # ...
```

**Key difference from before**: `_render_status_rule` receives the SAME `StatusRule` object in every language. The renderer only decides syntax. It cannot skip rules, add extra rules, or check different statuses.

### 6. Cross-Flow References (Unified)

The current codebase duplicates `_DYNAMIC_FIELDS` 5 times. The new design has ONE source of truth in `scenarios.yaml` (`use_from_previous`) and per-language `_make_field_reference()` methods:

```
BEFORE (5 independent dicts, can drift):
  _DYNAMIC_FIELDS         → Python syntax
  _DYNAMIC_FIELDS_JS      → JS syntax
  _DYNAMIC_FIELDS_KT      → Kotlin syntax (multi-line builder)
  _DYNAMIC_FIELDS_RS      → Rust struct syntax
  _DYNAMIC_FIELDS_RS_JSON → Rust JSON macro syntax

AFTER (1 spec + 4 mechanical translators):
  scenarios.yaml:
    use_from_previous: ["connector_transaction_id"]
    depends_on: "authorize"

  PythonRenderer._make_field_reference()   → "authorize_response.connector_transaction_id"
  JSRenderer._make_field_reference()       → "authorizeResponse.connectorTransactionId"
  KotlinRenderer._make_field_reference()   → "authorizeResponse.connectorTransactionId"
  RustRenderer._make_field_reference()     → "&authorize_response.connector_transaction_id"
```

For complex cases (Kotlin nested builders for mandate references), `_make_field_reference` can return multi-line strings — the method signature allows this. The complexity is contained in one method per language, not scattered across dict entries.

### 7. Structural Completeness Check

After rendering, verify that all languages produced structurally identical output (not by parsing ASTs, but by counting):

```python
# core/validator.py

@dataclass
class RenderManifest:
    """What a renderer claims it produced — checked post-render."""
    scenario_key: str
    language: str
    flow_count: int
    status_checks: dict[str, int]  # flow_name -> number of status checks
    return_field_count: int
    cross_flow_refs: list[str]     # ["capture.connector_transaction_id"]

class BaseRenderer(ABC):
    # ... existing methods ...

    def get_manifest(self) -> RenderManifest:
        """Return structural summary of what was rendered."""
        status_checks = {}
        for flow in self.scenario.flows:
            status_checks[flow.name] = len(flow.status_handling)

        return RenderManifest(
            scenario_key=self.scenario.key,
            language=self.language_name,
            flow_count=len(self.scenario.flows),
            status_checks=status_checks,
            return_field_count=len(self.scenario.return_fields),
            cross_flow_refs=[
                f"{f.name}.{field}"
                for f in self.scenario.flows
                for field in f.use_from_previous
            ],
        )


def validate_structural_parity(manifests: list[RenderManifest]) -> list[str]:
    """
    Compare render manifests across languages.
    
    Catches: "JS emitted 2 status checks for authorize, Rust emitted 3"
    Does NOT require AST parsing — just compares counts from the renderers.
    """
    errors = []
    if len(manifests) < 2:
        return errors

    first = manifests[0]
    for other in manifests[1:]:
        if other.flow_count != first.flow_count:
            errors.append(
                f"{other.language} has {other.flow_count} flows, "
                f"{first.language} has {first.flow_count} "
                f"(scenario: {first.scenario_key})"
            )

        if other.status_checks != first.status_checks:
            for flow_name in set(first.status_checks) | set(other.status_checks):
                a = first.status_checks.get(flow_name, 0)
                b = other.status_checks.get(flow_name, 0)
                if a != b:
                    errors.append(
                        f"{other.language} has {b} status checks for {flow_name}, "
                        f"{first.language} has {a} "
                        f"(scenario: {first.scenario_key})"
                    )

        if other.return_field_count != first.return_field_count:
            errors.append(
                f"{other.language} returns {other.return_field_count} fields, "
                f"{first.language} returns {first.return_field_count} "
                f"(scenario: {first.scenario_key})"
            )

        if other.cross_flow_refs != first.cross_flow_refs:
            errors.append(
                f"{other.language} cross-flow refs {other.cross_flow_refs} != "
                f"{first.language} {first.cross_flow_refs} "
                f"(scenario: {first.scenario_key})"
            )

    return errors
```

## Data Models

```python
# core/models.py
from pydantic import BaseModel, Field
from typing import Optional, Any
from enum import Enum


# ── Behavioral Spec (from scenarios.yaml) ─────────────────────────────────────

class StatusAction(str, Enum):
    """What to do when a status matches."""
    ERROR = "error"
    RETURN_EARLY = "return_early"


class StatusRule(BaseModel):
    """
    One behavioral rule for response handling.
    
    This is the core consistency mechanism. All 4 languages receive
    the same StatusRule and must implement it — they differ only in
    syntax, not in which statuses are checked or what action is taken.
    """
    status: list[str]                          # ["FAILED", "AUTHORIZATION_FAILED"]
    action: StatusAction                       # error | return_early
    message: Optional[str] = None              # "Payment failed: {error}"
    return_fields: Optional[dict[str, str]] = None  # For return_early: field map


class FlowDefinition(BaseModel):
    """One flow within a scenario (from scenarios.yaml)."""
    name: str
    pm_type: Optional[str] = None
    pm_type_variants: Optional[list[str]] = None  # Try each, use first supported
    required: bool = True
    capture_method: Optional[str] = None       # "AUTOMATIC" or "MANUAL"
    depends_on: Optional[str] = None
    use_from_previous: list[str] = Field(default_factory=list)
    status_handling: list[StatusRule] = Field(default_factory=list)


class Scenario(BaseModel):
    """Abstract scenario — no connector-specific data."""
    key: str
    name: str
    description: str
    flows: list[FlowDefinition]
    return_fields: dict[str, str] = Field(default_factory=dict)

    @property
    def required_flows(self) -> list[str]:
        return [f.name for f in self.flows if f.required]


# ── Flow Graph (from field-probe, per connector) ─────────────────────────────

class FlowNodeType(str, Enum):
    PREREQUISITE = "prerequisite"   # create_access_token, create_customer, create_order
    ENTRY_POINT = "entry_point"     # authorize, setup_recurring, tokenize
    DEPENDENT = "dependent"         # capture, refund, void, get, recurring_charge


class FieldProvider(BaseModel):
    """A field that a flow provides in its response."""
    response_path: str              # "connector_transaction_id" or "mandate_reference.connector_mandate_id.connector_mandate_id"
    description: str                # "Stripe PaymentIntent ID (pi_xxx)"


class FieldRequirement(BaseModel):
    """A field that a flow requires from a prior flow's response."""
    from_flow: str                  # "authorize"
    from_field: str                 # Key in the source flow's "provides"
    request_path: str               # Path in this flow's request where the value goes


class FlowNode(BaseModel):
    """One flow in the connector's flow graph."""
    type: FlowNodeType
    description: str = ""
    provides: dict[str, FieldProvider] = Field(default_factory=dict)
    requires: dict[str, FieldRequirement] = Field(default_factory=dict)


class FlowEdge(BaseModel):
    """Directed edge: from_flow must complete before to_flow can run."""
    from_flow: str                  # "authorize"
    to_flow: str                    # "capture"


class FlowGraph(BaseModel):
    """
    Connector-specific flow dependency graph.
    
    Generated by field-probe. Captures:
    - Which flows exist and their type (prerequisite, entry_point, dependent)
    - What each flow provides (response fields usable by downstream flows)
    - What each flow requires (fields from upstream flows)
    - Edges defining valid ordering
    
    This is the single source of truth for multi-step flow linking.
    It replaces the 5 hardcoded _DYNAMIC_FIELDS dicts and the
    dependency knowledge scattered across generator, test suite, and router.
    """
    nodes: dict[str, FlowNode]     # flow_key -> FlowNode
    edges: list[FlowEdge]


# ── Field Link (resolved at hydration time from flow_graph) ───────────────────

class FieldLink(BaseModel):
    """
    Resolved field dependency for a specific flow in a hydrated scenario.
    
    Created by the hydrator from the flow_graph's requires/provides.
    The renderer uses this to generate cross-flow variable references
    in language-specific syntax.
    """
    from_flow: str                  # "authorize"
    from_field: str                 # "connector_transaction_id"
    target_path: str                # Path in the request payload to substitute


# ── Hydrated Models (probe data + behavioral spec + flow graph resolved) ──────

class FlowAvailability(str, Enum):
    SUPPORTED = "supported"
    PARTIALLY_SUPPORTED = "partially_supported"


class HydratedFlow(BaseModel):
    """Flow with actual payload from probe + behavioral spec + resolved field links."""
    name: str
    payload: dict[str, Any]                    # From probe (pure data, no markers)
    status_handling: list[StatusRule]           # From scenario spec
    field_links: dict[str, FieldLink] = Field(default_factory=dict)  # From flow_graph
    depends_on: Optional[str] = None
    use_from_previous: list[str] = Field(default_factory=list)
    capture_method: Optional[str] = None
    is_prerequisite: bool = False              # True for auto-discovered prereq flows


class HydratedScenario(BaseModel):
    """Scenario ready for rendering — probe data + behavioral spec + flow graph combined."""
    key: str
    name: str
    description: str
    connector_name: str
    prerequisite_flows: list[HydratedFlow] = Field(default_factory=list)  # From flow_graph
    flows: list[HydratedFlow]
    return_fields: dict[str, str]              # From scenario spec
    availability: FlowAvailability = FlowAvailability.SUPPORTED
    skipped_optional: list[str] = Field(default_factory=list)
```

## Directory Structure

```
scripts/generators/docs/
├── __main__.py                 # CLI entry point
├── config/
│   ├── connectors.yaml         # Display names
│   ├── payment_methods.yaml    # PM categories
│   └── status_mapping.yaml     # Per-language status/error/return templates
├── specs/
│   └── scenarios.yaml          # Behavioral specs (status_handling + return_fields)
├── core/
│   ├── models.py               # Pydantic models (StatusRule, HydratedScenario, etc.)
│   ├── loader.py               # Probe data + scenario loading
│   ├── hydrator.py             # Scenario + probe → HydratedScenario
│   └── validator.py            # Structural completeness checks
├── generators/
│   ├── markdown.py             # Markdown doc generation
│   └── llms_txt.py             # LLM index generation
├── renderers/
│   ├── base.py                 # BaseRenderer with shared render() loop
│   ├── python.py               # _render_status_rule, _render_return, etc.
│   ├── javascript.py
│   ├── kotlin.py
│   └── rust.py
├── templates/
│   ├── connector.md.j2
│   └── all_connector.md.j2
└── utils/
    ├── line_numbers.py
    └── syntax_check.py
```

## Execution Flow

```
1. Load specs/scenarios.yaml → list[Scenario]
   (Each scenario has flows with status_handling rules + return_fields)

2. Load config/status_mapping.yaml → per-language templates

3. For each connector:
   a. Load field-probe data (including flow_graph)
   b. Parse flow_graph → FlowGraph (nodes, edges, provides/requires)
   c. Hydrate scenarios:
      i.   Resolve prerequisites from flow_graph (e.g., create_access_token for paypal)
      ii.  Resolve field links from flow_graph (replaces _DYNAMIC_FIELDS)
      iii. Fill payloads from probe data
      iv.  Skip unsupported scenarios
      → list[HydratedScenario] (each with prerequisite_flows + field_links)

4. For each language:
   a. Create renderer with hydrated scenarios + status mapping
   b. Render each scenario → code string
      - Prerequisites rendered as setup steps (with comments explaining why)
      - Field links rendered as cross-flow variable references
      - Status rules rendered as language-specific error/return handling
   c. Collect RenderManifest from each render

5. Validate structural parity across languages
   (Same flow count, same status check count, same return fields, same field links)

6. Generate markdown docs (rendered code embedded via templates)

7. Run syntax checks on generated code
```

## Migration Strategy

### Phase 1: Extract Configuration (Safe, No Behavior Change)
1. Move `_DISPLAY_NAMES` → `config/connectors.yaml`
2. Move `_PROBE_PM_BY_CATEGORY` → `config/payment_methods.yaml`
3. Update `generate.py` to load from config files
4. **Verify**: `diff` generated output before/after — must be identical

### Phase 2: Generate Flow Graphs in Field-Probe
1. Add `flow_graph` generation to field-probe pipeline:
   - For each supported flow, create a node with type (prerequisite/entry_point/dependent)
   - Detect `provides` from proto response schemas
   - Detect `requires` by matching request fields against known provider fields
   - Mark dependencies (access tokens, session state)
   - Build edges from requires relationships
2. Write `flow_graph` section into each connector's probe JSON
3. **Verify**:
   - Every connector with `create_access_token` has edges from it to authorize
   - Every connector with `capture` has a `requires.connector_transaction_id.from_flow = "authorize"` entry
   - Compare against the hardcoded `_DYNAMIC_FIELDS` — flow_graph must capture every relationship that `_DYNAMIC_FIELDS` hardcodes
   - Validate: `connector_test_suites/*/suite_spec.json` dependencies should be a subset of flow_graph edges

### Phase 3: Define Behavioral Specs
1. Create `core/models.py` with `StatusRule`, `FlowDefinition`, `Scenario`, flow graph models, etc.
2. Create `specs/scenarios.yaml` — extract from `_FALLBACK_SCENARIO_GROUPS` + hardcoded status handling from all four `_scenario_step_*` functions
3. Create `config/status_mapping.yaml` — extract from hardcoded strings in each renderer
4. **Verify**: Load and validate scenarios parse correctly with Pydantic

### Phase 4: Build Hydration Layer
1. Create `core/hydrator.py` with `ScenarioHydrator`
2. Integrate flow_graph:
   - `_resolve_prerequisites()` — walk graph to discover prerequisite flows
   - `_resolve_field_links()` — replace hardcoded `_DYNAMIC_FIELDS` with graph lookups
3. Handle PM variant resolution (replaces `pm_key_variants` logic in `detect_scenarios`)
4. Handle missing flows (return None for unsupported, track skipped optional)
5. **Verify**:
   - Hydrator produces same scenario list as current `detect_scenarios()` for all connectors
   - PayPal scenarios include `create_access_token` as prerequisite
   - Razorpay scenarios include `create_order` as prerequisite
   - Field links match what `_DYNAMIC_FIELDS` currently hardcodes

### Phase 5: Build Renderers
1. Create `renderers/base.py` with the shared `render()` loop
2. Implement `_render_status_rule()` in each language — translate from `StatusRule`, not from `if flow_key ==` branches
3. Implement `_render_return()` in each language — translate from `return_fields`, not from `if scenario.key ==` branches
4. Implement `_render_field_link()` — translates `FieldLink` to language-specific variable reference (replaces all 5 `_DYNAMIC_FIELDS_*` dicts)
5. Implement `_render_prerequisite()` — renders prerequisite flows with explanatory comments
6. Implement `_render_request()` — payload serialization with proper language-specific handling
7. **Verify**: Generated output matches current output for all connectors (or is intentionally improved for consistency)

### Phase 6: Validation & Cleanup
1. Implement `validate_structural_parity()` — run post-render, now also checks field_links and prerequisites
2. Wire renderers into markdown generation pipeline
3. Remove old `_scenario_step_*`, `_scenario_return_*`, `_DYNAMIC_FIELDS_*`
4. Remove `set_scenario_groups()` global state
5. **Verify**: Full pipeline produces consistent output, structural validator passes

## Why This Fixes the Inconsistencies

| Current Problem | Root Cause | How This Fixes It |
|----------------|------------|-------------------|
| JS runs 2 scenarios, Rust runs 4 | Each `_scenario_step_*` has different `if flow_key ==` branches | All languages receive same `HydratedScenario` list from hydrator |
| JS authorize gives success, Kotlin gives FAILED | Different status enum comparisons across languages | `StatusRule.status = ["FAILED", "AUTHORIZATION_FAILED"]` — all languages check the same values |
| JS throws on 400, Python doesn't | Status handling independently coded per language | `StatusRule.action = "error"` — all languages must implement the same error action |
| Rust checks `RefundStatus::RefundFailure`, others check `"FAILED"` | Rust renderer has different flow_key branches | `status_mapping.yaml` maps abstract status to per-language enum; spec defines which statuses to check |
| Rust checks `status_code >= 400` for auth flows, no other language does | Hardcoded in `_rust_status_check_lines` only | If it should be checked, it goes in `status_handling`; if not, it's removed from Rust too |
| Return fields differ across languages | 4 independent `_scenario_return_*` if/elif chains | `return_fields` defined once in scenario spec |
| Cross-flow refs can drift | 5 copies of `_DYNAMIC_FIELDS` | `flow_graph.requires` is the single source; one `_render_field_link()` per language |
| PayPal examples fail without access token | Generator doesn't know about prerequisite flows | `flow_graph` has `create_access_token` as prerequisite; hydrator auto-prepends it |
| Razorpay examples fail without order | Generator doesn't know about create_order | `flow_graph` has `create_order` as prerequisite; hydrator auto-prepends it |
| No docs on "what to call before capture" | Flow dependencies hardcoded in 3 disconnected places | `flow_graph` is machine-readable, per-connector, generated by field-probe |

## Design Decisions

### Why StatusRule instead of Assertion?

The generated code doesn't contain test assertions (`assert`, `assertEquals`). It contains **status-driven control flow** — `if/raise`, `if/throw`, `when/throw`, `match/Err`. `StatusRule` accurately describes what the spec captures: "when you see this status, take this action."

### Why keep per-language renderers instead of a template engine?

Each language has genuinely different patterns:
- **Python**: `ParseDict({...}, proto_message)` with `raise RuntimeError`
- **JavaScript**: Object literals with TypeScript types, `throw new Error`
- **Kotlin**: `.newBuilder().apply { }` builders with `throw RuntimeException`
- **Rust**: `serde_json::json!({})` with `match` and `return Err()`

A string template can't express Kotlin's nested builder pattern or Rust's match arms. Per-language `_render_*` methods preserve idiomatic output while the behavioral spec guarantees consistency.

### Why not markers in probe data?

Probe data represents actual request payloads captured from connector integrations. Injecting `__FROM_PREVIOUS__` markers would couple probe data to the rendering system. Instead, `flow_graph.requires` defines the dependencies and the renderer resolves them at render time. Probe data stays pure.

### Why structural validation instead of AST parsing?

Parsing Python, JavaScript, Kotlin, and Rust ASTs to verify consistency is enormously complex and fragile. Counting status checks, return fields, and cross-flow refs from the render manifest is simple, reliable, and catches the exact class of bugs we're seeing (missing status checks, extra error handling in one language, different return shapes).

### Why generate the flow graph in field-probe?

Three options were considered:

1. **Hand-write flow_graph per connector** — Accurate but doesn't scale to 60+ connectors and will drift.
2. **Generate from proto metadata** — Proto files define field types but not flow relationships. Not enough info.
3. **Generate in field-probe** (chosen) — Field-probe already runs each flow, knows which are supported, and has access to proto schemas. It can detect dependencies by matching request fields against known response fields from other flows. The `context_deferred_paths` in `scenario_api.rs` and the `suite_spec.json` dependencies serve as validation baselines.

