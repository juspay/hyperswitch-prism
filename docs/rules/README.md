# Documentation Rules

This directory contains the rules and prompts for generating consistent API reference documentation from proto files.

## Purpose

These rules ensure that all API reference documentation follows a consistent format, making it easy for developers to understand and use the Universal Prism (UCS) APIs. The rules cover:

- Front matter format for metadata
- Business-focused overview sections
- Developer-centric scenarios tables
- Complete field documentation (no omissions)
- Working code examples with proper authentication

## Usage

### Generating Documentation for a New RPC

1. Read the proto file to understand the request/response message definitions
2. Copy the **Generate Prompt** (below) into your LLM tool (Claude, Codex, etc.)
3. Replace the placeholders:
   - `{service_name}` - The proto service name (e.g., `PaymentService`)
   - `{rpc_name}` - The RPC name (e.g., `Authorize`)
   - `{request_proto}` - The request message definition
   - `{response_proto}` - The response message definition
4. Run the prompt and save the output to `docs/api-reference/services/{service}/{rpc}.md`

### Updating Field Tables After Proto Changes

1. Read the existing markdown file and the updated proto file
2. Copy the **Update Fields Prompt** (below) into your LLM tool
3. Replace the placeholders with the existing markdown and new proto definitions
4. Run the prompt and replace only the Request/Response Fields sections in the target file

## Generate Prompt

```
You are a technical documentation writer for a payment processing platform.

TASK: Generate complete API reference documentation for the following gRPC RPC.

PROTO SERVICE: {service_name}
PROTO RPC: {rpc_name}

REQUEST MESSAGE:
```protobuf
{request_proto}
```

RESPONSE MESSAGE:
```protobuf
{response_proto}
```

DOCUMENTATION RULES:
{rules_content}

OUTPUT FORMAT:
Produce a complete markdown file with the following sections:

1. **Front matter** (wrapped in HTML comments <!-- --- ... --- -->)
2. **## Overview** - Business use case from a digital business point of view (e-commerce, marketplaces, SaaS)
3. **## Purpose** - Why use this RPC? Include scenarios table with columns: Scenario | Developer Implementation
4. **## Request Fields** - Complete table with ALL fields from proto (Rule 8.1)
5. **## Response Fields** - Complete table with ALL fields from proto (Rule 8.1)
6. **## Example** - grpcurl command and JSON response (Rule 6.1)
7. **## Next Steps** - Relative links to related operations

REQUIREMENTS:
- Include EVERY field from the proto messages in the tables (no omissions)
- Use proper markdown table format
- Include Type, Required, and Description columns
- Description should be clear and developer-focused
- **Stripe Authentication (Rule 6.1):**
  - Use headers:
    - `-H "x-connector: stripe"` (specifies the connector)
    - `-H "x-connector-config: {\"config\":{\"Stripe\":{\"api_key\":\"$STRIPE_API_KEY\"}}}"` (authentication)
  - NOT `Authorization: Bearer` format
  - Developer should be able to set STRIPE_API_KEY and run the command directly
- Example must use realistic test data (Rule 6.2)
```

## Update Fields Prompt

```
You are a technical documentation writer for a payment processing platform.

TASK: Update ONLY the Request Fields and Response Fields sections of an existing API reference document.

EXISTING DOCUMENT:
```markdown
{existing_markdown}
```

CURRENT PROTO DEFINITIONS:

REQUEST MESSAGE:
```protobuf
{request_proto}
```

RESPONSE MESSAGE:
```protobuf
{response_proto}
```

RULES:
1. Update ONLY the "## Request Fields" and "## Response Fields" sections
2. Include ALL fields from the proto messages (do not omit any)
3. Match the existing table format
4. Keep all other sections exactly as they are
5. Do not modify Overview, Purpose, Example, or Next Steps sections

OUTPUT:
Provide only the updated Request Fields and Response Fields sections.
```

## Files

- **[rules.md](./rules.md)** - Complete documentation rules specification

## Example Output

See existing documentation for reference:
- [Authorize](../api-reference/services/payment-service/authorize.md)
- [Capture](../api-reference/services/payment-service/capture.md)
- [Get](../api-reference/services/payment-service/get.md)
- [Void](../api-reference/services/payment-service/void.md)
