# Grace 

AI-powered connector code generation and payment integration toolkit.

## Quick Reference

### Available Commands

| Command | Purpose | Example |
|---------|---------|---------|
| `grace techspec <connector>` | Generate technical specification | `grace techspec stripe -f ./docs` |
| `grace integrate <connector>` | Run full integration workflow | `grace integrate stripe --flow Authorize` |

### Orchestrator Workflow Files

| File | Purpose | When to Use |
|------|---------|-------------|
| `workflow/1_orchestrator.md` | Batch connector flow implementation | Implement flow across multiple connectors |
| `workflow/2_connector.md` | Per-connector agent | Single connector full implementation |
| `workflow/2.1_links.md` | Links discovery agent | Find connector documentation URLs |
| `workflow/2.2_techspec.md` | Tech spec generation agent | Generate connector spec |
| `workflow/2.3_codegen.md` | Code generation agent | Implement and test connector code |
| `workflow/2.4_pr.md` | PR creation agent | Commit and create pull request |
| `workflow/3_task_orchestrator.md` | Task-based orchestrator | Specific configuration changes |
| `workflow/4_task_implementer.md` | Task implementation agent | Single task execution |
| `workflow/ORCHESTRATOR_TEMPLATE.md` | Template for new orchestrators | Creating custom orchestrators |

### Common Operations

**Generate a tech spec:**
```bash
grace techspec <connector-name> -f /path/to/api-docs -v
```

**Implement a new connector:**
```
integrate <ConnectorName> using grace/rulesbook/codegen/.gracerules
```

**Add a missing flow to existing connector:**
```
add Refund flow to <Connector> using grace/rulesbook/codegen/.gracerules_add_flow
```

**Add payment methods:**
```
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to <Connector> using grace/rulesbook/codegen/.gracerules_add_payment_method
```

**Batch process multiple connectors:**
```
Implement {FLOW} for all connectors in {CONNECTORS_FILE}. Read grace/workflow/1_orchestrator.md and follow it exactly.
Integration details: {CONNECTORS_FILE}
Branch: {BRANCH}
```

**Process configuration tasks:**
```
Process all tasks in grace/tasks.json. Read grace/workflow/3_task_orchestrator.md and follow it exactly.
Branch: {BRANCH}
```

---

## Installation

```bash
cd grace
uv sync  # if uv not installed: pip install uv
source .venv/bin/activate
```

## Quick Start

### 1. Generate Tech Spec

```bash
# From local docs folder (PDF)
grace techspec <connector-name> -f /path/to/api-docs -v

# Or from a URL
grace techspec <connector-name> -e
```

Output: `rulesbook/codegen/references/specs/<connector-name>.md`

### 2. Run Code Generation

Go back to the connector-service root folder (not `grace/`).

Open `connector-service/` in your AI coding agent and run:

```
integrate <ConnectorName> using grace/rulesbook/codegen/.gracerules
```

The AI agent will run through these phases:
1. **Foundation** → scaffolds files, auth, module registration
2. **Authorize** → payment authorization flow
3. **PSync** → payment status sync
4. **Capture** → capture authorized payments
5. **Refund** → full & partial refunds
6. **RSync** → refund status sync
7. **Void** → cancel authorized payments
8. **Quality** → scores implementation (must be ≥ 60)

### 3. Verify Build

```bash
cargo build
```

### Other Commands

Add a missing flow:
```
add Refund flow to <Connector> using grace/rulesbook/codegen/.gracerules_add_flow
```

Add payment methods:
```
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to <Connector> using grace/rulesbook/codegen/.gracerules_add_payment_method
```

---

## Orchestrator Workflow (Batch Processing)

For implementing a payment flow across multiple connectors in one run:

```
Implement {FLOW} for all connectors in {CONNECTORS_FILE}. Read grace/workflow/1_orchestrator.md and follow it exactly.
Integration details: {CONNECTORS_FILE}
Branch: {BRANCH}
```

**Example:**
```
Implement GooglePay for all connectors in connectors.json. Read grace/workflow/1_orchestrator.md and follow it exactly.
Integration details: connectors.json
Branch: feat/google_pay_impl
```

### Workflow Architecture

```
workflow/
├── 1_orchestrator.md      # Top-level orchestrator
├── 2_connector.md         # Per-connector agent
├── 2.1_links.md          # Links discovery
├── 2.2_techspec.md       # Tech spec generation
├── 2.3_codegen.md        # Code generation
├── 2.4_pr.md             # PR creation
├── 3_task_orchestrator.md # Task-based orchestrator (NEW)
└── 4_task_implementer.md  # Task implementation agent (NEW)
```

---

## Task-Based Workflow (NEW)

For implementing specific connector configuration tasks (e.g., enable 3DS, add payment methods, fix mappings):

### 1. Edit `tasks.json` with your tasks

```json
{
  "tasks": [
    {
      "connector_name": "nuvei",
      "connector_account_details": {
        "auth_type": "SignatureKey",
        "api_key": "...",
        "key1": "...",
        "api_secret": "..."
      },
      "payment_method": "card",
      "payment_method_type": "credit",
      "prompt": "Enable 3DS flow for Nuvei credit card payments and map the response fields correctly."
    }
  ]
}
```

### 2. Run the task orchestrator

Tell Claude Code:
```
Process all tasks in grace/tasks.json. Read grace/workflow/3_task_orchestrator.md and follow it exactly.
Branch: feat/my-connector-changes
```

The orchestrator will:
1. Read all tasks from `tasks.json`
2. Process each task sequentially
3. For each task, spawn a Task Implementation Agent that will:
   - Analyze the prompt
   - Find the connector files
   - Implement the requested changes
   - Build and test via grpcurl
   - Commit changes
4. Report summary of all tasks

### Task File Format

| Field | Required | Description |
|-------|----------|-------------|
| `connector_name` | Yes | Name of the connector (e.g., `nuvei`, `stripe`) |
| `connector_account_details` | Yes | Auth credentials object |
| `connector_account_details.auth_type` | Yes | `SignatureKey`, `HeaderKey`, `BodyKey`, etc. |
| `connector_account_details.api_key` | Usually | API key for the connector |
| `connector_account_details.key1` | Sometimes | Additional key (if required) |
| `connector_account_details.api_secret` | Sometimes | API secret (if required) |
| `payment_method` | Yes | Payment method type (e.g., `card`, `wallet`, `bank_debit`) |
| `payment_method_type` | Yes | Specific type (e.g., `credit`, `debit`, `google_pay`) |
| `prompt` | Yes | Natural language description of changes to make |

---



---

## Enhanced Features

### Custom Prompt Framework

Define custom prompts for specific payment flows and connectors:

1. **Configure prompts** in `grace/config/prompts.yaml`:
```yaml
task_prompts:
  three_ds:
    name: "3DS Flow Implementation"
    applicable_flows: ["ThreeDS"]
    prompt: |
      ## 3DS Guidelines
      1. Extract browser_info: accept_header, user_agent (REQUIRED)
      ...
```

2. **Reference in tasks.json**:
```json
{
  "tasks": [{
    "connector_name": "worldpay",
    "payment_method": "card",
    "custom_prompt_config": "three_ds"
  }]
}
```

3. **Run with enhanced orchestrator**:
```
Process all tasks in grace/tasks.json. Read grace/workflow/3_task_orchestrator.md and follow it exactly.
Branch: feat/my-changes
```

See [docs/ENHANCED_FEATURES.md](docs/ENHANCED_FEATURES.md) for complete documentation.

---

See [setup.md](setup.md) for detailed setup instructions, API key configuration, and advanced usage.
