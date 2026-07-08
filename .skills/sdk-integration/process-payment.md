# Skill: Process Payment

## Description
Create a complete payment authorization flow using the Prism SDK.

## When to Use
- Implementing payment checkout
- Adding charge/purchase functionality
- Testing payment flows

## Parameters
- language: Programming language
- payment_method: card, wallet, bank_transfer, etc.
- amount: Payment amount (will convert to minorAmount)
- currency: ISO currency code (USD, EUR, GBP, etc.)
- capture_method: AUTOMATIC or MANUAL

## Prompt Template

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
