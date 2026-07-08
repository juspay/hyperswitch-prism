# Skill: Handle Payment Errors

## Description
Implement robust error handling for payment operations with Prism SDK.

## When to Use
- Adding error handling to payment flows
- Debugging payment failures
- Implementing retry logic

## Parameters
- language: Programming language
- operation: authorize, capture, refund, void, etc.

## Prompt Template

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
