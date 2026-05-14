# Comprehensive 10XGRACE to Grace Workflow Migration Plan

## 1. Executive Summary

Transform 10XGRACE into a Grace-compatible workflow system for implementing payment method support across multiple connectors.

**Goal**: Automate adding Card/Wallet/BankTransfer payment methods to Stripe/Adyen/Checkout connectors.

**Approach**: Mirror Grace's exact architecture:

- Orchestrator coordinates multiple connectors
- Connector Agent handles each connector end-to-end
- Subagents handle specs and implementation
- Sequential execution only
- Simple JSON array input

---

## 2. Grace Workflow Deep Dive

### 2.1 Grace's Purpose

Located in `/Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism/grace/workflow/`

Grace automates adding payment flows to connectors:

- Input: Payment flow type + list of connectors
- Process: Documentation discovery → Tech spec → Code generation → Testing → PR
- Output: Implemented connector with new payment flow

### 2.2 Grace Architecture

```
Level 1: Orchestrator (1_orchestrator.md)
├── Reads: connectors.json ["Adyen", "Stripe"]
├── Variable: FLOW="BankDebit"
├── Actions:
│   ├── Parse connectors.json
│   ├── Check creds.json for credentials
│   └── For each connector:
│       └── Spawn Connector Agent (wait for result)
│
Level 2: Connector Agent (2_connector.md)
├── Variable: CONNECTOR="Stripe"
├── Actions:
│   ├── Spawn Links Agent - Find API docs
│   ├── Spawn Tech Spec Agent - Generate spec
│   ├── Spawn Codegen Agent - Write code
│   ├── Run cargo build
│   ├── Run grpcurl tests
│   └── Spawn PR Agent - Git commit
```

### 2.3 Grace File Format

**connectors.json**:

```json
["Adyen", "Stripe", "Checkout", "Braintree"]
```

**creds.json** (already exists):

```json
{
  "stripe": { "api_key": "sk_test_xxx" },
  "adyen": { "api_key": "AQExxx" }
}
```

### 2.4 Grace Execution Flow

```
1. User invokes via Task tool:
   Task(
     prompt="Read grace/workflow/1_orchestrator.md
     Variables: FLOW=BankDebit, CONNECTORS_FILE=connectors.json"
   )

2. Orchestrator loads connectors: ["Adyen", "Stripe"]

3. Orchestrator checks credentials:
   - Adyen: exists in creds.json -> proceed
   - Stripe: exists in creds.json -> proceed

4. Orchestrator spawns Connector Agent for Adyen:
   - Links Agent finds docs
   - Tech Spec generates spec
   - Codegen implements code
   - Build + test
   - PR created
   - Returns: SUCCESS

5. Orchestrator waits, then spawns Connector Agent for Stripe:
   - Same process
   - Returns: SUCCESS

6. Orchestrator prints summary
```

---

## 3. 10XGRACE Migration to Grace Pattern

### 3.1 10XGRACE's New Purpose

10XGRACE will implement payment method support (not flows):

- **Payment Method**: Card, Wallet, BankTransfer, BNPL
- **Connectors**: Stripe, Adyen, Checkout, etc.
- **Example**: "Implement Card payment method support for Stripe"

### 3.2 10XGRACE Architecture (Identical to Grace)

```
Level 1: Orchestrator (10xgrace/workflow/1_orchestrator.md)
├── Reads: connectors.json ["Stripe", "Adyen"]
├── Variable: PAYMENT_METHOD="Card"
├── Actions:
│   ├── Parse connectors.json
│   ├── Check creds.json
│   └── For each connector:
│       └── Spawn Connector Agent (wait)
│
Level 2: Connector Agent (10xgrace/workflow/2_connector.md)
├── Variable: CONNECTOR="Stripe"
├── Actions:
│   ├── Spawn Requirements Agent - Analyze connector
│   ├── Spawn Tech Spec Agent - Generate payment method spec
│   ├── Spawn Implementation Agent - Add Card support
│   ├── Run npm run re:build
│   ├── Run E2E tests
│   └── Spawn PR Agent
```

### 3.3 10XGRACE vs Grace Comparison

| Aspect      | Grace           | 10XGRACE (New)          |
| ----------- | --------------- | ------------------- |
| Domain      | Payment flows   | Payment methods     |
| Variable    | FLOW            | PAYMENT_METHOD      |
| Input       | connectors.json | connectors.json     |
| Credentials | creds.json      | creds.json          |
| Language    | Rust            | TypeScript/ReScript |
| Build       | cargo build     | npm run re:build    |
| Test        | grpcurl         | Cypress/Playwright  |

---

## 4. Detailed Implementation

### 4.1 File Structure

```
/Users/tushar.shukla/Downloads/Work/euler-ucs/hyperswitch-prism/
├── 10xgrace/
│   └── workflow/
│       ├── 1_orchestrator.md
│       ├── 2_connector.md
│       ├── 2.1_requirements.md
│       ├── 2.2_techspec.md
│       ├── 2.3_implementation.md
│       └── 2.4_pr.md
├── grace/
│   └── workflow/
│       ├── 1_orchestrator.md
│       └── ...
├── connectors.json
└── creds.json
```

### 4.2 Workflow File Specifications

#### 1_orchestrator.md

```markdown
# Orchestrator Agent

Implement {PAYMENT_METHOD} support across connectors.

## Inputs

| Variable          | Description               | Example           |
| ----------------- | ------------------------- | ----------------- |
| {PAYMENT_METHOD}  | Payment method to add     | Card, Wallet      |
| {CONNECTORS_FILE} | JSON file with connectors | connectors.json   |
| {BRANCH}          | Git branch                | feat/card-support |

## STEP 0: Parse Connectors

cat {CONNECTORS_FILE} | jq '.[]' -r

## STEP 1: Pre-flight

pwd && ls package.json
git checkout -b {BRANCH}
cat creds.json

## STEP 2: Process Each Connector

For each connector in list:
Check credentials in creds.json
If missing: mark SKIPPED
Else: spawn Connector Agent

## Connector Agent Spawn

Task(
description="Add {PAYMENT_METHOD} to {CONNECTOR}",
prompt="Read 10xgrace/workflow/2_connector.md
Variables:
CONNECTOR: <name>
PAYMENT_METHOD: {PAYMENT_METHOD}
BRANCH: {BRANCH}"
)

## Summary Report

Print SUCCESS/FAILED/SKIPPED per connector
```

#### 2_connector.md

```markdown
# Connector Agent

Add {PAYMENT_METHOD} support to {CONNECTOR}.

## Phase 1: Requirements

Spawn 2.1_requirements.md
Variables: CONNECTOR, PAYMENT_METHOD

## Phase 2: Tech Spec

Spawn 2.2_techspec.md
Variables: CONNECTOR, PAYMENT_METHOD, REQUIREMENTS_PATH

## Phase 3: Implementation

Spawn 2.3_implementation.md
Variables: CONNECTOR, PAYMENT_METHOD, TECHSPEC_PATH

## Phase 4: Testing

Run: npm run re:build
Run: npm run cy:run
Run: npm run pw:test

## Phase 5: PR

Spawn 2.4_pr.md
Variables: CONNECTOR, FILES_CHANGED

## Report

CONNECTOR: {CONNECTOR}
STATUS: SUCCESS|FAILED
PR: <url>
```

#### 2.1_requirements.md

```markdown
# Requirements Agent

Analyze {CONNECTOR} for {PAYMENT_METHOD} implementation.

## Steps

1. Find connector files:
   glob: src/connectors/{connector}\*/\*\*

2. Check current payment methods supported

3. Identify files needing modification

4. Check existing patterns

## Output

{
"connectorFiles": [...],
"currentMethods": [...],
"filesToModify": [...],
"patterns": [...]
}
```

#### 2.2_techspec.md

```markdown
# Tech Spec Agent

Generate spec for adding {PAYMENT_METHOD} to {CONNECTOR}.

## Inputs

- Requirements from 2.1
- Connector documentation (if available)

## Output

Payment method spec with:

- API endpoints
- Request/response transformers
- Error handling
- File modifications needed
```

#### 2.3_implementation.md

```markdown
# Implementation Agent

Implement {PAYMENT_METHOD} support for {CONNECTOR}.

## Inputs

- Tech spec from 2.2
- Existing connector code

## Steps

For each file in spec:

1. Read current contents
2. Generate modifications
3. Write new contents

## Rules

- Match existing code style
- Use TypeScript/ReScript patterns
- Complete implementations only
```

#### 2.4_pr.md

```markdown
# PR Agent

Create PR for {CONNECTOR} {PAYMENT_METHOD} support.

## Steps

1. Stage files:
   git add src/connectors/{connector}\*

2. Commit:
   git commit -m "feat({connector}): add {payment_method} support"

3. Push and create PR

## Rules

- Only stage connector-specific files
- Scrub credentials from code
```

---

## 5. Execution Example

### Scenario: Add Card Support to Multiple Connectors

**Step 1: Create connectors.json**

```json
["Stripe", "Adyen", "Checkout"]
```

**Step 2: Invoke via Task tool**

```
Task(
  description="Add Card support to connectors",
  prompt="Read 10xgrace/workflow/1_orchestrator.md

Variables:
  PAYMENT_METHOD: Card
  CONNECTORS_FILE: connectors.json
  BRANCH: feat/card-payment-method"
)
```

**Step 3: Orchestrator Execution**

```
→ Load connectors: ["Stripe", "Adyen", "Checkout"]
→ Check creds.json: all have credentials
→ Spawn Connector Agent for Stripe
  → Requirements: analyze Stripe connector
  → Tech Spec: Card payment method spec
  → Implementation: add Card support
  → Testing: npm run re:build + E2E
  → PR: created
  → Result: SUCCESS
→ Spawn Connector Agent for Adyen
  → ...same process...
  → Result: SUCCESS
→ Spawn Connector Agent for Checkout
  → ...same process...
  → Result: SUCCESS
→ Summary: 3 SUCCESS, 0 FAILED, 0 SKIPPED
```

---

## 6. Timeline

### Week 1: Create Workflow Files

**Day 1-2**: 1_orchestrator.md

- Write orchestrator workflow
- Define variables and phases
- Document Task tool invocation

**Day 3-4**: 2_connector.md

- Write connector agent workflow
- Define 5 phases
- Document subagent spawning

**Day 5**: 2.1_requirements.md + 2.2_techspec.md

- Requirements discovery workflow
- Tech spec generation workflow

### Week 2: Implementation and Testing

**Day 1-2**: 2.3_implementation.md + 2.4_pr.md

- Code generation workflow
- Git/PR workflow

**Day 3-4**: Integration Testing

- Test with sample connectors
- Verify sequential execution
- Check credential handling

**Day 5**: Documentation and Polish

- Final workflow reviews
- Usage documentation
- Migration guide

---

## 7. Success Criteria

- [ ] 6 workflow files in `10xgrace/workflow/`
- [ ] Uses same connectors.json format as Grace
- [ ] Reads from shared creds.json
- [ ] Sequential connector processing
- [ ] Each connector: Requirements → Spec → Implementation → Test → PR
- [ ] Git workflow with scoped commits
- [ ] Summary report generation
- [ ] No CLI - Task tool only

---

## 8. Integration with Existing Grace

Both Grace and 10XGRACE workflows coexist:

```
hyperswitch-prism/
├── grace/workflow/     # Payment flows (BankDebit, Wallet)
├── 10xgrace/workflow/      # Payment methods (Card, BankTransfer)
├── connectors.json     # Shared connector list
└── creds.json          # Shared credentials
```

Usage:

- Grace: `FLOW=BankDebit, CONNECTORS_FILE=connectors.json`
- 10XGRACE: `PAYMENT_METHOD=Card, CONNECTORS_FILE=connectors.json`

---

## 9. Risks and Mitigation

| Risk                | Mitigation                                     |
| ------------------- | ---------------------------------------------- |
| File conflicts      | Sequential execution (one connector at a time) |
| Missing credentials | Silent skip with logging                       |
| Build failures      | Retry logic with max attempts                  |
| Git conflicts       | Scoped commits per connector                   |

---

## 10. Conclusion

This migration transforms 10XGRACE from a standalone frontend tool into a Grace-compatible workflow for payment method implementation. The architecture mirrors Grace exactly, enabling consistent automation across both payment flows and payment methods.

**Key Achievement**: Unified workflow system for all connector automation needs.
