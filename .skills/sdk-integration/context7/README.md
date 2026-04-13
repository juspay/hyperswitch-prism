# Context7 Skills for Hyperswitch Prism

This directory contains YAML skill definitions compatible with the Context7 registry.

## Available Skills

| Skill | File | Description |
|-------|------|-------------|
| Setup Payment Client | `prism-setup-payment-client.yaml` | Initialize PaymentClient for any connector |
| Process Payment | `prism-process-payment.yaml` | Complete payment authorization flow |
| Handle Errors | `prism-handle-errors.yaml` | Error handling for all operations |
| Route Connectors | `prism-route-connectors.yaml` | Dynamic provider switching |
| Configure Connector | `prism-configure-connector.yaml` | Connector-specific configuration |
| Process Refund | `prism-process-refund.yaml` | Full and partial refunds |

## Publishing to Context7 Registry

### Option 1: Submit via GitHub Issue

1. Fork the [Context7 registry repository](https://github.com/upstash/context7)
2. Add your skills to the appropriate directory
3. Submit a Pull Request

### Option 2: Submit via Context7 Dashboard

1. Go to https://context7.com/add-library
2. Select "Submit Skills"
3. Upload YAML files or provide GitHub repo URL
4. Wait for review

### Option 3: Self-Host (Advanced)

Host skills in your own registry:

```bash
# Create a skills registry repo
# Structure: skills/{category}/{skill-name}.yaml

# Users install via:
npx ctx7 skills install github.com/juspay/prism-skills/prism-setup-payment-client
```

## Skill Format Reference

```yaml
name: "skill-name"           # Unique identifier
version: "1.0.0"            # Semantic versioning
description: "..."          # Short description
author: "..."               # Author/organization
tags: [...]                 # Categories for discovery

parameters:                 # Input parameters
  - name: param_name
    type: string|number|boolean|array
    required: true|false
    description: "..."
    enum: [...]             # Optional allowed values
    default: ...            # Optional default value

prompt: |                   # The actual prompt template
  Instructions here...
  Use {{parameter_name}} for interpolation

examples:                   # Example usages
  - parameters:
      param: value
    description: "..."
```

## Testing Skills Locally

Before submitting, test your skills:

```bash
# Install Context7 CLI
npm install -g @upstash/context7-mcp

# Test a skill
ctx7 skills test prism-setup-payment-client.yaml \
  --param language=node \
  --param connector=stripe \
  --param environment=sandbox
```

## Updating Skills

1. Update version number (follow semver)
2. Update changelog in skill file
3. Submit updated YAML
4. Context7 will version and deprecate old skills automatically

## Need Help?

- Context7 Docs: https://context7.com/docs
- Skill Schema: https://context7.com/schema/skill.json
- Support: https://github.com/upstash/context7/issues
