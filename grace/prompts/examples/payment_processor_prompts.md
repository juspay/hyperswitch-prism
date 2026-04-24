# Example: Payment Processor Custom Prompts

This file demonstrates how to create custom prompts for payment processor integrations.

## Use Case: Adding a New Payment Processor

When integrating a new payment processor (e.g., a regional gateway), you can provide
custom prompts to guide the AI through processor-specific requirements.

### Custom Prompt Configuration

```yaml
# In grace/config/prompts.yaml, add to connector_prompts section:

my_processor:
  name: "MyProcessor Integration Guidelines"
  description: "Custom handling for MyProcessor API"
  prompt: |
    ## MyProcessor-Specific Implementation Notes

    1. **Authentication**: Use X-API-Key header with HMAC-SHA256 signature
    2. **Request Format**: All requests must include:
       - merchantId (from credentials)
       - timestamp (ISO 8601)
       - signature (HMAC of merchantId + timestamp + body)
    3. **Response Handling**: Check responseCode field (not HTTP status)
       - 00 = Success
       - 01-99 = Specific error codes (see error mapping)
    4. **Webhook Verification**: Verify signature using webhook_secret
    5. **Idempotency**: Use X-Request-ID header for idempotency
```

### Task-Specific Override

For a specific task, you can override prompts inline in tasks.json:

```json
{
  "tasks": [
    {
      "connector_name": "myprocessor",
      "connector_account_details": { ... },
      "payment_method": "card",
      "payment_method_type": "credit",
      "prompt": "Implement card payments with 3DS support...",
      "custom_prompt_override": "grace/prompts/custom/myprocessor_3ds.md"
    }
  ]
}
```

### Prompt Template Variables

Use these variables in your custom prompts - they will be substituted at runtime:

| Variable | Description | Example |
|----------|-------------|---------|
| `{CONNECTOR}` | Lowercase connector name | `myprocessor` |
| `{CONNECTOR_NAME}` | Original casing | `MyProcessor` |
| `{FLOW}` | Payment flow | `Authorize`, `ThreeDS` |
| `{PAYMENT_METHOD}` | Payment method type | `card`, `wallet` |
| `{PAYMENT_METHOD_TYPE}` | Specific type | `credit`, `apple_pay` |
| `{BRANCH}` | Git branch | `feat/myprocessor-3ds` |

### Example: 3DS Flow Custom Prompt

```markdown
## {CONNECTOR} 3DS Implementation

When implementing 3DS for {CONNECTOR}:

1. **Device Data Collection**:
   - Send browser_info to {CONNECTOR}'s /3ds/device endpoint
   - Include threeDSMethodNotificationURL for callback

2. **Challenge Flow**:
   - If response indicates challenge required:
     - Extract challenge_url and creq (challenge request)
     - Build redirect_form with these fields
   - User completes challenge at issuer page
   - Callback received at return_url with cres

3. **Frictionless Flow**:
   - If risk assessment passes, direct authorization
   - No redirect needed

4. **Status Mapping**:
   | {CONNECTOR} Status | UCS Status |
   |-------------------|------------|
   | Y (authenticated) | AUTHORIZED |
   | N (failed)        | AUTHENTICATION_FAILED |
   | A (attempted)     | AUTHORIZED |
   | U (unavailable)   | AUTHENTICATION_FAILED |
   | C (challenge)     | AUTHENTICATION_PENDING |
```

### Example: Error Handling Custom Prompt

```markdown
## {CONNECTOR} Error Handling

### Error Response Format
{CONNECTOR} returns errors in this structure:
```json
{
  "responseCode": "XX",
  "responseMessage": "Description",
  "errors": [
    {"field": "fieldName", "message": "error description"}
  ]
}
```

### Error Code Mapping
Map these {CONNECTOR} codes to UCS PaymentError:
| {CONNECTOR} Code | UCS Error | Retryable |
|------------------|-----------|-----------|
| 01 | InvalidCardNumber | No |
| 02 | ExpiredCard | No |
| 03 | InsufficientFunds | No |
| 05 | TemporaryError | Yes |
| 12 | InvalidTransaction | No |
| 14 | InvalidCardNumber | No |
| 51 | InsufficientFunds | No |
| 54 | ExpiredCard | No |
| 65 | SoftDecline | Yes (with 3DS) |
| 96 | SystemError | Yes |
```

### Best Practices

1. **Keep prompts focused**: Each prompt should cover one aspect (auth, 3DS, errors)
2. **Use examples**: Include request/response examples where possible
3. **Document edge cases**: Note any unusual behavior or special handling
4. **Update as needed**: Refine prompts based on implementation experience
