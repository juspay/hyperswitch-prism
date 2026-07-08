# Skill: Configure Connector

## Description
Get the exact configuration required for a specific payment processor.

## When to Use
- Setting up a new payment provider
- Understanding required credentials
- Comparing connector capabilities

## Parameters
- connector: Payment provider name
- environment: sandbox or production
- features: List of needed features (cards, wallets, webhooks, etc.)

## Prompt Template

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
