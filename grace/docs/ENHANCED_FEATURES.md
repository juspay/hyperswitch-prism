# GRACE Enhanced Features Documentation

This document describes the enhancements made to the GRACE framework for custom prompt injection and improved orchestrator flexibility.

## Table of Contents

1. [Quick Start](#quick-start)
2. [Custom Prompt Framework](#custom-prompt-framework)
3. [Orchestrator Updates](#orchestrator-updates)
4. [Configuration Reference](#configuration-reference)
5. [Examples](#examples)
6. [Troubleshooting](#troubleshooting)

---

## Quick Start

### Using Custom Prompts

1. **Define custom prompts** in `grace/config/prompts.yaml`:

```yaml
task_prompts:
  my_custom_flow:
    name: "My Custom Flow"
    applicable_flows: ["CustomFlow"]
    prompt: |
      ## Implementation Guidelines
      1. Step one
      2. Step two
```

2. **Reference in tasks.json**:

```json
{
  "tasks": [{
    "connector_name": "myconnector",
    "payment_method": "card",
    "custom_prompt_config": "my_custom_flow"
  }]
}
```

3. **Run the orchestrator**:

```
Process all tasks in grace/tasks.json. Read grace/workflow/3_task_orchestrator.md and follow it exactly.
Branch: feat/my-changes
```

---

## Custom Prompt Framework

### Overview

The custom prompt framework allows you to:
- Define reusable prompt templates for different payment flows
- Provide connector-specific implementation guidelines
- Override default behavior with task-specific instructions

### File Structure

```
grace/
├── config/
│   ├── prompts.yaml          # Main configuration
│   └── prompt_schema.json    # Validation schema
└── prompts/
    ├── examples/             # Example prompts
    └── custom/               # Your custom prompts
```

### Prompt Types

#### 1. Default Prompts

Applied to all tasks unless overridden:

```yaml
defaults:
  system_context: |
    You are an expert payment integration engineer...
  code_style: |
    - Use macros for all connector implementations
    - Use RouterDataV2, not RouterData
```

#### 2. Task Prompts

Specific to payment flows or methods:

```yaml
task_prompts:
  three_ds:
    name: "3DS Flow Implementation"
    applicable_flows: ["ThreeDS", "3DS"]
    applicable_payment_methods: ["card"]
    prompt: |
      ## 3DS Guidelines
      ...
```

#### 3. Connector Prompts

Specific to individual connectors:

```yaml
connector_prompts:
  stripe:
    name: "Stripe Guidelines"
    prompt: |
      ## Stripe-Specific Notes
      ...
```

### Variable Substitution

Use these variables in your prompts:

| Variable | Description |
|----------|-------------|
| `{CONNECTOR}` | Lowercase connector name |
| `{CONNECTOR_NAME}` | Original casing |
| `{FLOW}` | Payment flow |
| `{PAYMENT_METHOD}` | Payment method type |
| `{PAYMENT_METHOD_TYPE}` | Specific type |
| `{BRANCH}` | Git branch name |

---

## Orchestrator Updates

### 3_task_orchestrator.md Changes

1. **Added Rule 10**: Task list source validation
2. **Added Rule 11**: Custom prompt support
3. **Added Step 2**: Load custom prompts before processing
4. **Enhanced**: Pre-flight stash command

### 4_task_implementer.md Changes

1. **Added Input**: `{CUSTOM_PROMPT}` parameter
2. **Enhanced Phase 1**: Apply custom prompt guidance during analysis
3. **Enhanced Phase 4**: Consider custom prompts during implementation

### New Template

Created `ORCHESTRATOR_TEMPLATE.md` for creating new orchestrators with:
- Standard structure
- Required rules
- Optional features (custom prompts, queue integration)
- Validation checklist

---

## Configuration Reference

### prompts.yaml Schema

```yaml
version: "1.0"                    # Schema version

# Default prompts for all tasks
defaults:
  system_context: "..."
  code_style: "..."

# Task-specific prompts
task_prompts:
  <key>:
    name: "Display Name"
    description: "What this prompt does"
    applicable_flows: ["Flow1", "Flow2"]
    applicable_payment_methods: ["method1"]
    prompt: "The actual prompt content"

# Connector-specific prompts
connector_prompts:
  <connector_name>:
    name: "Display Name"
    description: "What this prompt does"
    prompt: "The actual prompt content"

# Validation rules
validation:
  max_prompt_length: 10000
  required_fields: ["name", "description", "prompt"]

# Injection points
injection_points:
  - name: "pre_analysis"
    description: "Before task analysis"
    applicable_to: ["3_task_orchestrator.md"]
```

---

## Examples

### Example 1: 3DS Flow Custom Prompt

```yaml
task_prompts:
  three_ds:
    name: "3DS Flow Implementation"
    applicable_flows: ["ThreeDS"]
    prompt: |
      ## {CONNECTOR} 3DS Implementation

      ### Required Fields
      - browser_info.accept_header (REQUIRED)
      - browser_info.user_agent (REQUIRED)

      ### Response Handling
      Map these {CONNECTOR} statuses:
      - 3dsDeviceDataRequired → DEVICE_DATA_COLLECTION_PENDING
      - 3dsChallenged → AUTHENTICATION_PENDING
      - authorized → AUTHORIZED

      ### Redirect Form
      Build redirect_form with:
      - url: The 3DS server URL
      - form_fields: jwt, any additional required fields
```

### Example 2: Connector-Specific Prompt

```yaml
connector_prompts:
  adyen:
    name: "Adyen-Specific Guidelines"
    prompt: |
      ## Adyen Implementation Notes

      1. **Merchant Account**: Required in every request
      2. **Reference**: Provide unique reference for idempotency
      3. **Payment Method**: Use paymentMethod object with type field
      4. **Shopper Interaction**: Set to Ecommerce or ContAuth
```

### Example 3: tasks.json with Custom Prompt

```json
{
  "tasks": [
    {
      "connector_name": "worldpay",
      "connector_account_details": {
        "api_key": "...",
        "key1": "...",
        "auth_type": "BodyKey"
      },
      "payment_method": "card",
      "payment_method_type": "credit",
      "prompt": "Implement Worldpay connector support for Cards with 3DS",
      "custom_prompt_config": "three_ds"
    }
  ]
}
```

---

## Troubleshooting

### Custom Prompt Not Applied

**Problem**: Custom prompt not being used during implementation.

**Solution**:
1. Verify `custom_prompt_config` in tasks.json matches a key in `prompts.yaml`
2. Check that `prompts.yaml` is valid YAML (use a linter)
3. Ensure the orchestrator is reading the correct config file

### Variable Substitution Not Working

**Problem**: Variables like `{CONNECTOR}` appear literally in output.

**Solution**:
1. Verify variable names match allowed list
2. Check variable format: must be `{UPPERCASE}`
3. Ensure subagent is passing variables correctly

### Validation Errors

**Problem**: Error loading `prompts.yaml`.

**Solution**:
1. Check against `prompt_schema.json`
2. Ensure all required fields are present
3. Verify prompt length is under 10,000 characters

---

## Migration Guide

### From Legacy Tasks

If you have existing tasks without custom prompts:

1. They will continue to work with default prompts
2. Add `custom_prompt_config` to tasks that need specific guidance
3. Create custom prompts for reusable patterns

### Adding to Existing Orchestrators

To add custom prompt support to other orchestrators:

1. Add `CUSTOM_PROMPT` to inputs section
2. Add prompt loading step before processing
3. Pass custom prompt to subagents
4. Update subagents to apply custom guidance
