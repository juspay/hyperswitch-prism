---
name: sdk-integration
description: >
  Helps developers integrate with the Hyperswitch Prism SDK across different programming
  languages and payment scenarios. Provides skills for setting up payment clients,
  processing payments, handling errors, routing between connectors, configuring connectors,
  and processing refunds. Supports Python, Node.js, Java/Kotlin, and Rust SDKs.
license: Apache-2.0
compatibility: Works with any AI coding tool that can read files and generate code.
metadata:
  author: juspay
  version: "1.0"
  domain: sdk-integration
---

# Hyperswitch Prism SDK Integration Skills

These skills help developers integrate with the Hyperswitch Prism SDK across different programming languages and payment scenarios.

---

## Table of Contents

1. [Setup Payment Client](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/setup-payment-client.md)
2. [Process Payment](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/process-payment.md)
3. [Handle Payment Errors](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/handle-errors.md)
4. [Route Between Connectors](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/route-between-connectors.md)
5. [Configure Connector](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/configure-connector.md)
6. [Process Refund](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/process-refund.md)
7. [How to Use These Skills](#how-to-use-these-skills)
8. [Converting to Context7 Skills Format](#converting-to-context7-skills-format)

---

## 1. Setup Payment Client

**File:** [setup-payment-client.md](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/setup-payment-client.md)

Initialize and configure the Hyperswitch Prism PaymentClient for your chosen connector.

### When to Use
- Starting a new integration
- Adding a new payment processor
- Setting up connector credentials

### Parameters
- **language**: Programming language (python, node, java, rust)
- **connector**: Payment provider (stripe, adyen, braintree, paypal, etc.)
- **environment**: sandbox or production

### Prompt Template

Create a complete PaymentClient setup for {{connector}} in {{language}}.

Requirements:
1. Import/require the necessary modules
2. Configure the connector with environment variables
3. Initialize the PaymentClient
4. Include error handling for missing credentials
5. Add comments explaining each configuration option

For {{connector}}, include:
- Required credentials (API keys, merchant account, etc.)
- Optional configuration options
- Environment-specific settings ({{environment}})

Reference examples from {{connector}} examples directory if available.

Output format: Complete, runnable code snippet with imports and comments.

---

## 2. Process Payment

**File:** [process-payment.md](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/process-payment.md)

Create a complete payment authorization flow using the Prism SDK.

### When to Use
- Implementing payment checkout
- Adding charge/purchase functionality
- Testing payment flows

### Parameters
- **language**: Programming language
- **payment_method**: card, wallet, bank_transfer, etc.
- **amount**: Payment amount (will convert to minorAmount)
- **currency**: ISO currency code (USD, EUR, GBP, etc.)
- **capture_method**: AUTOMATIC or MANUAL

### Prompt Template

Create a complete payment authorization function in {{language}} using Hyperswitch Prism.

Payment Details:
- Method: {{payment_method}}
- Amount: {{amount}} {{currency}}
- Capture: {{capture_method}}

Requirements:
1. Create a PaymentClient with proper configuration
2. Build the PaymentServiceAuthorizeRequest with:
   - merchantTransactionId (unique ID generation)
   - Amount object with minorAmount calculation
   - Payment method details ({{payment_method}})
   - Appropriate captureMethod ({{capture_method}})
3. Call client.authorize()
4. Handle the response:
   - SUCCESS/CHARGED status
   - FAILED status with error details
   - PENDING status (if applicable)
5. Wrap in try-catch for IntegrationError and ConnectorError
6. Include example test card data for sandbox

Output: Complete function with comments explaining the flow.

---

## 3. Handle Payment Errors

**File:** [handle-errors.md](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/handle-errors.md)

Implement robust error handling for payment operations with Prism SDK.

### When to Use
- Adding error handling to payment flows
- Debugging payment failures
- Implementing retry logic

### Parameters
- **language**: Programming language
- **operation**: authorize, capture, refund, void, etc.

### Prompt Template

Create comprehensive error handling for {{operation}} operations in {{language}} using Hyperswitch Prism.

Requirements:
1. Wrap the {{operation}} call in try-catch/try-except
2. Handle specific error types:
   - IntegrationError: SDK/library issues, invalid requests
   - ConnectorError: Payment processor errors (declines, timeouts)
3. For IntegrationError:
   - Log detailed error message
   - Check for common causes (missing fields, invalid types)
4. For ConnectorError:
   - Extract error_code and error_message
   - Map to user-friendly messages
   - Determine if retry is appropriate
   - Handle specific decline reasons (insufficient_funds, expired_card, etc.)
5. Include retry logic for transient failures
6. Return structured error result

Provide examples of:
- Card declined
- Invalid API key
- Network timeout
- Validation error

Output: Error handling utility class/function with examples.

---

## 4. Route Between Connectors

**File:** [route-between-connectors.md](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/route-between-connectors.md)

Implement intelligent routing to switch between payment providers dynamically.

### When to Use
- Supporting multiple payment processors
- Implementing failover logic
- Optimizing for cost/routing rules

### Parameters
- **language**: Programming language
- **primary_connector**: Primary payment provider
- **fallback_connector**: Fallback provider (optional)
- **routing_criteria**: currency, region, amount_threshold, etc.

### Prompt Template

Create a dynamic routing implementation in {{language}} using Hyperswitch Prism.

Routing Logic:
- Primary: {{primary_connector}}
- Fallback: {{fallback_connector}}
- Criteria: {{routing_criteria}}

Requirements:
1. Define configuration objects for both connectors
2. Create a routing function that selects connector based on:
   {{routing_criteria}}
3. Common patterns to demonstrate:
   - Currency-based routing (USD → Stripe, EUR → Adyen)
   - Region-based routing
   - Amount-based routing (high value → specific processor)
   - Random/load balancing
4. Implement fallback logic:
   - Try primary connector
   - On specific failures, retry with fallback
   - Track which connector succeeded
5. Show how to switch connectors in ONE line of code

Output: Complete routing implementation with example criteria.

---

## 5. Configure Connector

**File:** [configure-connector.md](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/configure-connector.md)

Get the exact configuration required for a specific payment processor.

### When to Use
- Setting up a new payment provider
- Understanding required credentials
- Comparing connector capabilities

### Parameters
- **connector**: Payment provider name
- **environment**: sandbox or production
- **features**: List of needed features (cards, wallets, webhooks, etc.)

### Prompt Template

Provide complete connector configuration for {{connector}} in {{environment}} environment.

Features needed: {{features}}

Include:
1. Required credentials:
   - API keys, secrets, passwords
   - Merchant account IDs
   - Endpoint configurations
2. Optional settings:
   - Webhook endpoints
   - Timeout configurations
   - Retry policies
3. Sandbox test credentials (if applicable):
   - Test API keys
   - Test card numbers
   - Test amounts
4. Configuration validation:
   - Check all required fields are present
   - Validate credential formats
   - Environment-specific checks
5. Common pitfalls:
   - Credential permissions needed
   - IP whitelisting requirements
   - Webhook signature verification setup

Reference existing examples from examples/{{connector}}/ directory if available.

Output: Complete configuration object with comments and validation.

---

## 6. Process Refund

**File:** [process-refund.md](https://github.com/juspay/hyperswitch-prism/blob/main/.skills/sdk-integration/process-refund.md)

Implement refund operations using the Prism SDK.

### When to Use
- Adding refund functionality
- Handling partial refunds
- Processing cancellations

### Parameters
- **language**: Programming language
- **refund_type**: full or partial
- **original_amount**: Original transaction amount
- **refund_amount**: Amount to refund (for partial)

### Prompt Template

Create a complete refund processing function in {{language}} using Hyperswitch Prism.

Refund Details:
- Type: {{refund_type}}
- Original Amount: {{original_amount}}
- Refund Amount: {{refund_amount}}

Requirements:
1. Accept original transaction ID as input
2. Create RefundServiceRequest with:
   - merchantRefundId (unique)
   - paymentReference (original transaction)
   - amount (minorAmount for {{refund_type}} refund)
   - reason (optional but recommended)
3. Call appropriate refund method on client
4. Handle response statuses:
   - SUCCESS: Refund processed
   - PENDING: Refund queued
   - FAILED: Refund declined
5. Error handling for:
   - Invalid original transaction
   - Amount exceeds original
   - Connector refund failures
6. Include idempotency considerations

Output: Complete refund function with partial/full logic.

---

## How to Use These Skills

These skills can be:

1. **Published to Context7 Registry** - Making them discoverable by other developers
2. **Used internally** - Referenced by your AI coding assistant
3. **Converted to CLI skills** - Installed via `npx ctx7 skills install`

---

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
    enum: [python, node, java, rust]
  - name: connector
    type: string
  - name: environment
    type: string
    enum: [sandbox, production]
```

---

## Contributing

To add new skills:
1. Create a new `.md` file in this directory
2. Follow the template structure
3. Test with real SDK usage
4. Submit for review
