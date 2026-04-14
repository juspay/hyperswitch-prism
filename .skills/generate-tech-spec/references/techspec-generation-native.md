# Claude-Native Tech Spec Agent

You generate the tech spec for **{CONNECTOR}** without the grace CLI.
This must complete before any code work begins.

---

## Inputs

| Parameter | Description | Example |
|-----------|-------------|---------|
| `{CONNECTOR}` / `{Connector_Name}` | Connector name (exact casing) | `Adyen` |
| `{FLOW}` | Payment flow being implemented | `Card`, `BankDebit`, `Wallet` |

Use `{Connector_Name}` (exact casing) for the spec filename. Use `{connector}` (lowercase) for directory names.

---

## Step 1: Extract URLs

Read `data/integration-source-links.json` and extract the URL array for this connector.

```bash
# From connector-service root:
cat data/integration-source-links.json | jq -r '."{Connector_Name}" // ."{connector}" // empty | .[]' 2>/dev/null
```

If the file does not exist or has no entry for this connector, return FAILED with reason:
"No documentation links found for {CONNECTOR}. Run the Links Agent first."

---

## Step 2: Scrape Documentation

For each URL from Step 1, use the **WebFetch** tool to fetch the page content.

### Process:

1. Create the output directory: `grace/rulesbook/codegen/references/{connector}/`

2. For each URL (index `i` starting at 1):

   a. Call **WebFetch** with the URL and this prompt:
      ```
      Extract ALL technical API documentation content from this page.
      Include: endpoint URLs, HTTP methods, request/response JSON schemas,
      authentication details, headers, error codes, status codes, webhook
      information, and any code examples (especially curl).
      Preserve exact field names, types, and JSON structures.
      Return the content as structured markdown.
      ```

   b. Write the result to `grace/rulesbook/codegen/references/{connector}/source_{i}.md`

   c. If WebFetch fails for a URL (timeout, 403, etc.), log a warning and continue
      to the next URL

3. After scraping all URLs, verify at least one source file was created.
   If zero files were created, return FAILED with reason "Could not scrape any documentation URLs."

---

## Step 3: Generate Tech Spec

Read ALL source markdown files from `grace/rulesbook/codegen/references/{connector}/`.

Synthesize them into a single technical specification following this EXACT structure:

````markdown
# {ConnectorName} API Documentation

## Overview

**Connector Name:** {ConnectorName}
**API Version:** [extract from docs]
**Protocol:** REST
**Data Format:** JSON
**Architecture:** [extract from docs]

### Base URLs

| Environment | Endpoint URL |
|-------------|--------------|
| Sandbox | `https://...` |
| Live/Production | `https://...` |

### Additional Resources
- Documentation: [URL]
- API Reference: [URL]

---

## Authentication

### Method
[Extract exact auth method: Basic Auth, Bearer, API Key header, HMAC, etc.]

### Creating the Authentication Header
[Step-by-step instructions from docs]

### Sandbox Credentials
[If available in docs]

### API Key Permission Levels
[If documented]

---

## Common Headers

### Request Headers
| Header | Value | Required | Description |
|--------|-------|----------|-------------|
| ... | ... | ... | ... |

### Response Headers
| Header | Description |
|--------|-------------|
| ... | ... |

### cURL Example
[Base curl example with auth headers]

---

## HTTP Codes and Errors

### HTTP Status Codes
| Code | Definition | Explanation |
|------|------------|-------------|
| ... | ... | ... |

### Error Response Body Format
```json
{ ... exact error structure from docs ... }
```

### Error Codes
| Code | Description |
|------|-------------|
| ... | ... |

---

## Configuration Parameters

### Idempotent Requests
[If documented -- idempotency key header, mechanism]

### Rate Limits
[If documented]

---

## Complete Endpoint Inventory

### [Flow Name] (e.g., Authorizations / Payments)

#### 1. [Action Name] (e.g., Create an Authorization)
**Endpoint:** `[METHOD] [path]`
**Purpose:** [what this endpoint does]

**Request Headers:**
| Header | Value | Required |
|--------|-------|----------|
| ... | ... | ... |

**Request Body:**
```json
{
  "field1": "value",
  "field2": {
    "nested_field": "value"
  }
}
```

**Request Parameters:**
| Field | Type | Required | Description |
|-------|------|----------|-------------|
| field1 | string | Yes | Description |
| field2.nested_field | string | No | Description |

**Response [StatusCode] - [Status]:**

**Response Body:**
```json
{
  "id": "...",
  "status": "...",
  "amount": 0
}
```

**Response Fields:**
| Field | Type | Description |
|-------|------|-------------|
| id | string | Unique identifier |
| status | string | Transaction status |

[Repeat for EVERY endpoint found in docs:
 - Authorize / Create Payment
 - Capture
 - Refund / Reversal
 - Void / Cancel
 - Payment Sync (GET status)
 - Refund Sync (GET refund status)
 - Tokenization / Payment Instruments
 - Customer creation
 - Any other endpoints]

---

## Webhook Events

### Event Types
| Event | Description |
|-------|-------------|
| ... | ... |

### Webhook Payload Structure
```json
{ ... }
```

### Webhook Authentication & Signature Verification
[Method, headers, verification steps]

---

## Status Mappings

| Connector Status | Meaning |
|-----------------|---------|
| ... | ... |
````

### CRITICAL RULES for Step 3:
- Extract ALL endpoints found in the source documents, not just payment flows
- Use exact field names, JSON structures, and values from the source material
- Do NOT invent or assume any missing information
- Use "Not specified in source documentation" for relevant but missing info
- Include full request AND response JSON payloads for every endpoint
- Preserve original data types (integer, string, boolean, etc.)
- Make response bodies a one-to-one copy from the documentation

Write the spec to: `grace/rulesbook/codegen/references/specs/{ConnectorName}.md`

---

## Step 4: Enhance the Tech Spec

Re-read each source file from `grace/rulesbook/codegen/references/{connector}/`
and compare against the generated spec. For each source file:

1. Read the current tech spec from `grace/rulesbook/codegen/references/specs/{ConnectorName}.md`
2. Read one source file
3. Identify information present in the source but MISSING from the spec:
   - Additional endpoints not yet documented
   - Missing request/response fields
   - Authentication details not captured
   - Error codes or status mappings not listed
   - Webhook details not included
   - Configuration parameters not documented
   - curl examples not included
   - Sandbox/test credentials
4. Edit the tech spec file in-place to add the missing information
5. Move to the next source file

### Rules for enhancement:
- ALWAYS edit the spec file in-place -- never create a new document
- Process files ONE AT A TIME: Read source -> Edit spec -> next source
- Preserve existing correct information in the spec
- Add missing details, do not duplicate existing content
- Flag any conflicting information between source files with a `<!-- CONFLICT: ... -->` comment

---

## Step 5: Field Dependency and API Sequence Analysis

Read the enhanced tech spec and perform the following analysis, then APPEND the
results as new sections at the END of the tech spec file.

### 5a: Identify All API Flows

Read the tech spec and list every distinct API flow:
- Authorize, Capture, Refund, Void, Sale (direct charge)
- Payment Sync (GET status), Refund Sync
- Tokenization, Customer Creation, Session Creation
- Any other flows documented

### 5b: For Each Flow, Determine the API Call Sequence

For each flow, document the ordered sequence of API calls required:

```
Flow: [Flow Name]

Step 1: [METHOD] [endpoint]
        Input: field = USER_PROVIDED | from Step N response.field
        Returns: field_name (used in Step N+1)

Step 2: [METHOD] [endpoint]
        Input: field = from Step 1 response.field
        Returns: field_name (used in final step)

Step N (Final): [METHOD] [endpoint]
        Input: all fields with their sources
```

Show which fields come from previous steps and which are user-provided or configuration.

### 5c: Categorize Every Request Field

For each flow's final API call, categorize EVERY request field as:

- **USER_PROVIDED** -- comes directly from the merchant/user request
  (amount, currency, description, customer email, billing address, metadata, return_url, etc.)
- **PREVIOUS_API** -- comes from a response of a prior API call in the sequence
  (IDs, tokens, session keys)
  Include: Source API endpoint and source response field name
- **CONFIGURATION** -- static merchant/account settings, not per-transaction
  (merchant_id, API keys)
- **UNDECIDED** -- source unclear; prepare a specific question for clarification

### 5d: Create Field Dependency Map

For each flow, create a dependency chain:

```
[API Call 1]
  |-- response.field_a --> [API Call 2].request.field_x
  |-- response.field_b --> [Final API Call].request.field_y
[API Call 2]
  |-- response.field_c --> [Final API Call].request.field_z
```

### 5e: Document UNDECIDED Fields

For each UNDECIDED field, provide:
- Flow and API Call where it appears
- What the specification says about it
- A specific question with 2-3 options:
  a) Is it provided directly by the merchant?
  b) Does it come from calling a specific API? If so, which one?
  c) Is there another source?

### 5f: Write Analysis to Tech Spec

Append these sections to the END of the tech spec file using the Edit tool:

```markdown
---

## API Call Sequences

### [Flow Name]
[Ordered sequence as described in 5b]

[Repeat for each flow]

---

## Field Dependency Analysis

### [Flow Name]

| Field | Category | Reasoning | Source API | Source Response Field |
|-------|----------|-----------|------------|-----------------------|
| amount | USER_PROVIDED | Transaction amount from merchant | - | - |
| token | PREVIOUS_API | Must be created first | POST /tokenize | response.token |
| ... | ... | ... | ... | ... |

#### Prerequisite API Calls (in order)
1. [API Call] -- Purpose: [why], Provides: [fields]
2. [API Call] -- Purpose: [why], Provides: [fields], Depends on: Step 1

[Repeat for each flow]

---

## UNDECIDED Fields

### 1. [field_name] ({Flow} / {API Call})
**Specification says:** "[quote from spec]"
**Question:** Where does this field originate?
  a) Merchant-provided in the payment request
  b) From [Specific API] response
  c) Other source -- please specify
```

---

## Step 6: Verify Output

After all steps complete:

1. Verify the spec file exists at `grace/rulesbook/codegen/references/specs/{ConnectorName}.md`
2. Verify it contains all required sections:
   - Overview, Authentication, Common Headers, HTTP Codes
   - Complete Endpoint Inventory (with request AND response JSON for each endpoint)
   - API Call Sequences, Field Dependency Analysis, UNDECIDED Fields
3. Verify the spec has content in the endpoint sections (not just headers)

---

## Output

```
CONNECTOR: {CONNECTOR}
FLOW: {FLOW}
STATUS: SUCCESS | FAILED
TECHSPEC_PATH: <path to generated tech spec>
SOURCE_FILES: <number of source files scraped>
ENDPOINTS_DOCUMENTED: <count of endpoints in spec>
REASON: <details, if failed>
```

---

## Rules

1. **No code generation** -- this agent only produces the tech spec, nothing else
2. **Verify before reporting success** -- always confirm the file exists on disk
3. **Use WebFetch for all scraping** -- do not use curl, wget, or any other tool
4. **Sequential processing** -- scrape URLs one at a time, enhance one source file at a time
5. **Idempotent** -- if a tech spec already exists for this connector at the output path,
   report SUCCESS with the existing path (do not regenerate unless explicitly asked)
6. **URL source** -- URLs come from `data/integration-source-links.json`, written by the
   Links Agent. Do NOT expect or require an integration details file
7. **No grace CLI dependency** -- this agent must NOT use grace commands, Python venv,
   or any files from grace/.env
8. **Preserve exact data** -- copy request/response JSON structures exactly as documented,
   never invent field names or values
