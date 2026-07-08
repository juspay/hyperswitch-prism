# Skill: Process Refund

## Description
Implement refund operations using the Prism SDK.

## When to Use
- Adding refund functionality
- Handling partial refunds
- Processing cancellations

## Parameters
- language: Programming language
- refund_type: full or partial
- original_amount: Original transaction amount
- refund_amount: Amount to refund (for partial)

## Prompt Template

Create a complete refund processing function in {{language}} using Hyperswitch Prism.

Refund Details:
- Type: {{refund_type}}
{{#if partial}}
- Original Amount: {{original_amount}}
- Refund Amount: {{refund_amount}}
{{/if}}

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
