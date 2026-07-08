# Hyperswitch Prism SDK Integration Skills

These skills help developers integrate with the Hyperswitch Prism SDK across different programming languages and payment scenarios.

## Available Skills

### 1. Setup Payment Client
**File:** `setup-payment-client.md`

Initialize and configure the PaymentClient for your chosen connector.

**Use when:** Starting a new integration or adding a payment processor.

**Parameters:**
- `language`: python, node, java, php, rust
- `connector`: stripe, adyen, braintree, paypal, etc.
- `environment`: sandbox or production

---

### 2. Process Payment
**File:** `process-payment.md`

Create a complete payment authorization flow.

**Use when:** Implementing checkout or charge functionality.

**Parameters:**
- `language`: Programming language
- `payment_method`: card, wallet, bank_transfer
- `amount`: Payment amount
- `currency`: ISO currency code
- `capture_method`: AUTOMATIC or MANUAL

---

### 3. Handle Payment Errors
**File:** `handle-errors.md`

Implement robust error handling for all payment operations.

**Use when:** Adding error handling or debugging failures.

**Parameters:**
- `language`: Programming language
- `operation`: authorize, capture, refund, void

---

### 4. Route Between Connectors
**File:** `route-between-connectors.md`

Implement dynamic routing to switch payment providers.

**Use when:** Supporting multiple processors or failover logic.

**Parameters:**
- `language`: Programming language
- `primary_connector`: Main provider
- `fallback_connector`: Backup provider
- `routing_criteria`: currency, region, amount

---

### 5. Configure Connector
**File:** `configure-connector.md`

Get exact configuration for a specific payment processor.

**Use when:** Setting up a new provider or understanding credentials.

**Parameters:**
- `connector`: Payment provider name
- `environment`: sandbox or production
- `features`: Required features list

---

### 6. Process Refund
**File:** `process-refund.md`

Implement refund operations (full or partial).

**Use when:** Adding refund functionality.

**Parameters:**
- `language`: Programming language
- `refund_type`: full or partial
- `original_amount`: Original transaction amount
- `refund_amount`: Refund amount (for partial)

## How to Use These Skills

These skills can be:

1. **Published to Context7 Registry** - Making them discoverable by other developers
2. **Used internally** - Referenced by your AI coding assistant
3. **Converted to CLI skills** - Installed via `npx ctx7 skills install`

## Converting to Context7 Skills Format

To publish these to Context7, convert to their YAML format:

```yaml
name: "prism-setup-payment-client"
description: "Initialize Hyperswitch Prism PaymentClient"
prompt: |
  [Content from setup-payment-client.md]
parameters:
  - name: language
    type: string
    enum: [python, node, java, php, rust]
  - name: connector
    type: string
  - name: environment
    type: string
    enum: [sandbox, production]
```

## Contributing

To add new skills:
1. Create a new `.md` file in this directory
2. Follow the template structure
3. Test with real SDK usage
4. Submit for review
