# Grace 

AI-powered connector code generation and payment integration toolkit.

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
└── 2.4_pr.md             # PR creation
```

---



---

See [setup.md](setup.md) for detailed setup instructions, API key configuration, and advanced usage.
