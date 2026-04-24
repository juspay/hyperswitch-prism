# Grace CLI Setup Guide

## Prerequisites

- An AI API key (OpenAI, Anthropic, or any OpenAI-compatible provider)
- An AI coding agent (Cursor, Claude Code, or Windsurf)

---

## 1. Clone the Repos

Open Terminal and run:

```bash
git clone git@github.com:juspay/hyperswitch-prism.git
cd hyperswitch-prism
git clone git@github.com:juspay/grace.git
```

Your folder should now look like:

```
hyperswitch-prism/
├── crates/         ← Rust connector code
├── config/          ← Server configs
├── grace/           ← Grace CLI + codegen rules
└── ...
```

---

## 2. Install Grace

```bash
cd grace
uv sync  # if uv not installed: pip install uv
source .venv/bin/activate
grace --help
```

If you see a list of commands (`techspec`, `research`, `pr`...) — you're good!

> **Note:** You need to run `source .venv/bin/activate` every time you open a new terminal window. You'll see `(.venv)` at the start of your prompt when it's active.

---

## 3. Configure API Keys

```bash
cp .env.example .env
```

Open `.env` in any text editor and update these lines:

### Using GRID

```env
AI_API_KEY=your_api_key_here
AI_BASE_URL=https://grid.ai.juspay.net
AI_MODEL_ID=openai/qwen3-coder-480b
```

### Using OpenAI

```env
AI_API_KEY=sk-proj-...
AI_BASE_URL=https://api.openai.com/v1
AI_MODEL_ID=openai/gpt-4o
```

### Using Anthropic

```env
AI_API_KEY=sk-ant-...
AI_BASE_URL=https://api.anthropic.com/v1
AI_MODEL_ID=anthropic/claude-sonnet-4-20250514
```

### For URL Scraping

Set one of:

```env
FIRECRAWL_API_KEY=your_key_here
USE_PLAYWRIGHT=false
```

Save and close the file.

---

## 4. Generate a Tech Spec

### From a local docs folder (PDF)

```bash
grace techspec myconnector -f /path/to/api-docs -v
```

### From a URL

```bash
grace techspec myconnector -e
```

Replace `myconnector` with the actual name (e.g. `stripe`, `adyen`).

**Output:** `rulesbook/codegen/references/specs/myconnector.md`

---

## 5. Run Code Generation

**Important:** Go back to the connector-service root folder (not `grace/`)

Open `hyperswitch-prism/` in your AI coding agent (Cursor / Claude Code / Windsurf) and tell it:

```
integrate MyConnector using grace/rulesbook/codegen/.gracerules
```

The AI agent will automatically run through these phases:

1. **Foundation** → scaffolds files, auth, module registration
2. **Authorize** → payment authorization flow
3. **PSync** → payment status sync
4. **Capture** → capture authorized payments
5. **Refund** → full & partial refunds
6. **RSync** → refund status sync
7. **Void** → cancel authorized payments
8. **Quality** → scores implementation (must be ≥ 60)

This takes several minutes. Let it finish.

---

## 6. Verify the Build

From the connector-service root:

```bash
cargo build
```

No errors = your connector is ready!

---

## Other Useful Commands (for existing connectors)

### Add a missing flow

```
add Refund flow to Stripe using grace/rulesbook/codegen/.gracerules_add_flow
```

### Add multiple flows

```
add SetupMandate and RepeatPayment flows to Stripe using grace/rulesbook/codegen/.gracerules_add_flow
```

### Add payment methods (Category:Type syntax)

```
add Wallet:Apple Pay,Google Pay and Card:Credit,Debit to Stripe using grace/rulesbook/codegen/.gracerules_add_payment_method
```
