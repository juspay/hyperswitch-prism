# Skill: Setup Payment Client

## Description
Initialize and configure the Hyperswitch Prism PaymentClient for your chosen connector.

## When to Use
- Starting a new integration
- Adding a new payment processor
- Setting up connector credentials

## Parameters
- language: Programming language (python, node, java, php, rust)
- connector: Payment provider (stripe, adyen, braintree, paypal, etc.)
- environment: sandbox or production

## Prompt Template

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
