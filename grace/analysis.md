# API Field Dependency Analysis Rule Set

## Purpose
This document provides a step-by-step guided path for analyzing API connector flows and determining field dependencies across multiple API calls. 

---

## Core Principles

1. **Never make assumptions** - If a field's source is unclear, mark it as "UNDECIDED" and ask the user
2. **Trace backwards** - For each field, trace back through previous API calls to find its origin
3. **Document everything** - Maintain a complete mapping of field sources for each API flow
4. **Validate against documentation** - Cross-reference with connector output files and technical specs

---

## Step-by-Step Analysis Process

### STEP 1: Identify All API Flows

**Action:** Read the technical specification document and identify all distinct API flows.

**Process:**
1. Look for sections like "Payment Flow", "Authorization Flow", "Capture Flow", etc.
2. List each flow type (e.g., Authorize, Capture, Refund, Void, CreateCustomer, etc.)
3. Note the sequence of API calls for each flow

**Output Format:**
```
Flows Identified:
- Flow 1: [Flow Name] - [Brief Description]
- Flow 2: [Flow Name] - [Brief Description]
...
```

**Example:**
```
Flows Identified:
- Flow 1: Payment Authorization - Authorizes a payment transaction
- Flow 2: Payment Capture - Captures a previously authorized payment
- Flow 3: Customer Creation - Creates a new customer profile
```

---

### STEP 2: For Each Flow, Extract the Final API Call

**Action:** Identify the primary/final API call for each flow.

**Process:**
1. Determine which API call is the main objective of the flow
2. Note the API endpoint name/path
3. Document the request method (POST, GET, PUT, etc.)

**Output Format:**
```
Flow: [Flow Name]
Final API Call: [API Endpoint Name]
Method: [HTTP Method]
```

**Example:**
```
Flow: Payment Authorization
Final API Call: /v1/payments/authorize
Method: POST
```

---

### STEP 3: Extract All Request Fields for the Final API Call

**Action:** List every field in the request payload for the final API call.

**Process:**
1. Read the request schema/structure from the technical specification
2. List each field with its:
   - Field name
   - Data type
   - Whether it's required or optional
   - Brief description from the spec

**Output Format:**
```
Flow: [Flow Name]
API Call: [API Endpoint Name]

Request Fields:
1. field_name (type) [required/optional] - description
2. field_name (type) [required/optional] - description
...
```

**Example:**
```
Flow: Payment Authorization
API Call: /v1/payments/authorize

Request Fields:
1. amount (integer) [required] - Payment amount in smallest currency unit
2. currency (string) [required] - Three-letter ISO currency code
3. payment_method_token (string) [required] - Token representing the payment method
4. customer_id (string) [optional] - Unique customer identifier
5. order_id (string) [optional] - Merchant order reference
```

---

### STEP 4: Categorize Each Field by Source

**Action:** For each field, determine where it comes from.

**Categories:**
- **USER_PROVIDED** - Field comes directly from the user/merchant request
- **PREVIOUS_API** - Field comes from the response of a previous API call
- **UNDECIDED** - Source is unclear and requires user clarification

**Analysis Process for Each Field:**

1. **Check if it's payment/transaction data:**
   - Amount, currency, payment details → Usually USER_PROVIDED
   - Customer info provided by merchant → Usually USER_PROVIDED

2. **Check if it's an ID or token:**
   - Customer ID, Order ID, Payment Method Token → Check if there's a creation API
   - Session tokens, access tokens → Usually from PREVIOUS_API

3. **Search previous API calls in the flow:**
   - Look for API calls like CreateCustomer, CreateOrder, CreateToken, etc.
   - Check if the field appears in the response of those APIs

4. **Search connector output files:**
   - Check `/mnt/user-data/uploads/output/[connector-name]` for response schemas
   - Look for fields that match the required field name

5. **If still unclear:**
   - Mark as UNDECIDED
   - Add specific question for user clarification

**Output Format:**
```
Flow: [Flow Name]
API Call: [API Endpoint Name]

Field Source Analysis:

1. field_name
   Category: [USER_PROVIDED | PREVIOUS_API | UNDECIDED]
   Reasoning: [Why this categorization]
   Source API (if PREVIOUS_API): [API endpoint name]
   Response Field (if PREVIOUS_API): [field name in response]

2. field_name
   Category: [USER_PROVIDED | PREVIOUS_API | UNDECIDED]
   Reasoning: [Why this categorization]
   ...
```

**Example:**
```
Flow: Payment Authorization
API Call: /v1/payments/authorize

Field Source Analysis:

1. amount
   Category: USER_PROVIDED
   Reasoning: Transaction amount is provided by merchant at time of payment request

2. currency
   Category: USER_PROVIDED
   Reasoning: Currency is specified by merchant based on transaction context

3. payment_method_token
   Category: PREVIOUS_API
   Reasoning: Token must be created before authorization
   Source API: POST /v1/payment-methods/tokenize
   Response Field: token

4. customer_id
   Category: PREVIOUS_API
   Reasoning: Customer must be created in connector system first
   Source API: POST /v1/customers
   Response Field: customer_id

5. order_id
   Category: UNDECIDED
   Reasoning: Unclear if this is merchant's internal order ID or requires CreateOrder API call
   Question: Does order_id come from merchant's system or does it require calling a CreateOrder API first?
```

---

### STEP 5: Identify Prerequisite API Calls

**Action:** List all API calls that must be executed before the final API call.

**Process:**
1. Review all fields marked as PREVIOUS_API
2. List the source APIs identified in Step 4
3. Determine the sequence/order of prerequisite calls
4. Check for nested dependencies (e.g., CreatePaymentMethod might need CreateCustomer first)

**Output Format:**
```
Flow: [Flow Name]
Final API Call: [API Endpoint]

Prerequisite API Calls (in order):
1. [API Call Name]
   Purpose: [Why it's needed]
   Provides: [List of fields it provides for downstream calls]
   
2. [API Call Name]
   Purpose: [Why it's needed]
   Provides: [List of fields it provides for downstream calls]
   Dependencies: [Other prerequisite calls it depends on]
   
...
```

**Example:**
```
Flow: Payment Authorization
Final API Call: POST /v1/payments/authorize

Prerequisite API Calls (in order):
1. POST /v1/auth/token
   Purpose: Obtain access token for API authentication
   Provides: access_token (for Authorization header)
   
2. POST /v1/customers
   Purpose: Create customer record in connector system
   Provides: customer_id (for authorize request)
   Dependencies: Requires access_token from step 1
   
3. POST /v1/payment-methods/tokenize
   Purpose: Tokenize payment method details
   Provides: payment_method_token (for authorize request)
   Dependencies: Requires access_token from step 1, optionally customer_id from step 2
   
4. POST /v1/payments/authorize
   Purpose: Final authorization call
   Requires: access_token, payment_method_token, customer_id
```

---

### STEP 6: Create Field Dependency Map

**Action:** Create a comprehensive dependency map showing the flow of data through all API calls.

**Output Format:**
```
Flow: [Flow Name]

Complete Field Dependency Map:

API Call 1: [API Name]
├─ Request Fields:
│  ├─ field1: USER_PROVIDED
│  └─ field2: USER_PROVIDED
└─ Response Fields:
   ├─ response_field1 → Used in API Call 2 as [field_name]
   └─ response_field2 → Used in API Call 3 as [field_name]

API Call 2: [API Name]
├─ Request Fields:
│  ├─ field1: FROM API Call 1 (response_field1)
│  ├─ field2: USER_PROVIDED
│  └─ field3: UNDECIDED
└─ Response Fields:
   └─ response_field1 → Used in Final API Call as [field_name]

Final API Call: [API Name]
└─ Request Fields:
   ├─ field1: USER_PROVIDED
   ├─ field2: FROM API Call 2 (response_field1)
   └─ field3: FROM API Call 1 (response_field1)
```

---

### STEP 7: Document UNDECIDED Fields

**Action:** Compile all fields marked as UNDECIDED and prepare questions for the user.

**Output Format:**
```
UNDECIDED FIELDS - Require User Clarification

Flow: [Flow Name]
API Call: [API Endpoint]

Field: [field_name]
Description: [From spec]
Question: [Specific question about source]
Options:
- Is this provided by the merchant/user directly?
- Does this require a previous API call? If so, which one?
- Is this obtained from a different source? Please specify.

---

Field: [field_name]
...
```

**Example:**
```
UNDECIDED FIELDS - Require User Clarification

Flow: Payment Authorization
API Call: POST /v1/payments/authorize

Field: order_id
Description: Merchant order reference
Question: Where does order_id come from?
Options:
- Is this the merchant's internal order ID passed directly?
- Does this require calling a CreateOrder API first?
- Is this obtained from a different source? Please specify.

Additional Context Needed:
- Does the connector provide an Order Creation API?
- Should we check for any CreateOrder endpoints in the documentation?
```

---

### STEP 8: Generate Summary Document

**Action:** Create a final summary document with all analysis results.

**File Naming Convention:**
`[ConnectorName]_[FlowName]_Field_Dependency_Analysis.md`

**Document Structure:**
```markdown
# [Connector Name] - [Flow Name] Field Dependency Analysis

## Flow Overview
[Brief description of the flow]

## API Call Sequence
1. [API Call 1] - [Purpose]
2. [API Call 2] - [Purpose]
...
N. [Final API Call] - [Purpose]


## Field Source Mapping

### USER_PROVIDED Fields
Fields that come directly from merchant/user request:
- field1: [Description]
- field2: [Description]
...

### PREVIOUS_API Fields
Fields that come from previous API call responses:
- field1: Source API: [API], Response Field: [field], Description: [Description]
- field2: Source API: [API], Response Field: [field], Description: [Description]
...

### UNDECIDED Fields
Fields requiring clarification:
- field1: [Question/Context]
- field2: [Question/Context]
...

## Complete Dependency Chain
[Insert the full dependency map from Step 6]

## Questions for User
[Insert all questions from Step 7]

## References
- Technical Specification: [Document name/location]
- Connector Output Files: [List relevant files checked]
- Additional Documentation: [Any other docs referenced]
```
### STEP 9 : UPDATE THE TECHSPEC WITH THE SEQUENCE OF API CALLS 
 Find the generathed tech spec and update the tech spec with the sequence of api calls. 
---

## Detailed Analysis Guidelines

### How to Identify USER_PROVIDED Fields

**Common USER_PROVIDED Fields:**
- `amount` - Transaction amount
- `currency` - Currency code
- `description` - Payment description
- `customer_email` - Customer email
- `customer_name` - Customer name
- `billing_address` - Billing address details
- `shipping_address` - Shipping address details
- `metadata` - Custom merchant metadata
- `merchant_reference` - Merchant's internal reference
- `return_url` - URL for redirects
- `webhook_url` - Notification endpoint

**Indicators:**
- Fields that contain business/transaction data
- Fields that vary per transaction
- Fields that the merchant controls
- Fields described as "merchant provided" in specs

---

### How to Identify PREVIOUS_API Fields

**Common PREVIOUS_API Fields:**
- `customer_id` - Usually from CreateCustomer API
- `payment_method_id` / `payment_method_token` - Usually from TokenizePaymentMethod API
- `order_id` - May come from CreateOrder API (or could be USER_PROVIDED)
- `session_id` / `session_token` - Usually from CreateSession API
- `access_token` / `api_key` - From authentication endpoints
- `transaction_id` - From previous payment operations
- `authorization_id` - From authorize call (used in capture)
- `connector_customer_id` - From customer creation in connector system

**How to Verify:**
1. Search technical spec for related API calls
2. Check connector output files for response schemas
3. Look for patterns like:
   - "First, create a [resource], then use the returned [field]..."
   - "Requires a [field] obtained from [API]..."
   - "Use the [field] from the [API] response..."

**Search Locations:**
- `/mnt/user-data/uploads/output/[connector-name]/` - Response schemas
- Technical specification sections on prerequisites
- API flow diagrams
- Authentication sections

---

### How to Handle UNDECIDED Fields

**When to Mark as UNDECIDED:**
- Field could logically come from multiple sources
- Specification doesn't clearly indicate the source
- No matching field found in previous API responses
- Field name is ambiguous (e.g., "reference", "id", "token")

**What to Include in Questions:**
1. **Context:** Quote the field description from spec
2. **Specific Question:** Ask about the exact source
3. **Options:** Provide 2-3 likely scenarios
4. **Additional Info Requested:** Ask for relevant documentation

**Example Question Template:**
```
Field: [field_name]
Specification Says: "[exact quote from spec]"

Question: Where does this field originate?
a) Is it provided directly by the merchant in the payment request?
b) Does it come from calling [Specific API]? If so, what's the response field name?
c) Is there another API call we're missing that provides this?

Please provide:
- Documentation for any prerequisite API calls
- The response schema showing where this field appears
- Any flow diagrams showing the data flow
```

---

## Special Cases and Edge Cases

### Case 1: Optional Fields
**Handling:** Still categorize the source, but note that it's optional.
```
Field: customer_id
Category: PREVIOUS_API (optional)
Note: If provided, must come from CreateCustomer API. If not provided, connector creates anonymous transaction.
```

### Case 2: Fields with Multiple Sources
**Handling:** Document all possible sources.
```
Field: customer_identifier
Category: MULTIPLE_SOURCES
Option 1: USER_PROVIDED - Merchant's internal customer ID
Option 2: PREVIOUS_API - customer_id from POST /v1/customers
Reasoning: Spec indicates either can be used. Need clarification on which is preferred.
```

### Case 3: Conditional Fields
**Handling:** Document the condition.
```
Field: installment_plan_id
Category: PREVIOUS_API (conditional)
Condition: Required only if payment_type = "INSTALLMENT"
Source API: POST /v1/installment-plans
Response Field: plan_id
```

### Case 4: Derived Fields
**Handling:** Document how they're derived.
```
Field: authorization_header
Category: DERIVED
Source: Constructed as "Bearer " + access_token
Where access_token comes from: POST /v1/auth/token → response.access_token
```

### Case 5: Configuration Fields
**Handling:** Mark as configuration.
```
Field: merchant_id
Category: CONFIGURATION
Reasoning: Static merchant configuration, not transaction-specific
Source: Merchant account settings / connector configuration
```

---

## Cross-Reference Checklist

Before finalizing analysis, verify:

- [ ] Checked technical specification for all flows
- [ ] Reviewed connector output files in `/mnt/user-data/uploads/output/[connector]/`
- [ ] Searched for all CreateXXX, TokenizeXXX, AuthXXX APIs
- [ ] Identified all authentication/authorization mechanisms
- [ ] Mapped all ID and token fields to their source APIs
- [ ] Documented all UNDECIDED fields with specific questions
- [ ] Created dependency chain showing data flow
- [ ] Generated individual analysis file for each flow
- [ ] Compiled master summary with all questions for user

---

## Output File Structure

Generate the following files:

```
/mnt/user-data/outputs/
├── [ConnectorName]_Field_Dependency_Analysis/
│   ├── 00_MASTER_SUMMARY.md
│   ├── 01_[FlowName]_Analysis.md
│   ├── 02_[FlowName]_Analysis.md
│   ├── ...
│   ├── UNDECIDED_FIELDS_Questions.md
│   └── COMPLETE_DEPENDENCY_MAP.md
```

### 00_MASTER_SUMMARY.md
- Overview of all flows
- Summary statistics (total fields, USER_PROVIDED count, PREVIOUS_API count, UNDECIDED count)
- Quick reference table

### [FlowName]_Analysis.md
- Detailed analysis for specific flow
- All fields categorized
- Dependency chain for that flow

### UNDECIDED_FIELDS_Questions.md
- All questions compiled
- Organized by flow
- Ready to send to user

### COMPLETE_DEPENDENCY_MAP.md
- Visual representation of all flows
- Complete data flow diagram
- Cross-flow dependencies

---

## Validation Rules

Before finalizing:

1. **Completeness Check:**
   - Every field in every API call is categorized
   - No fields left unanalyzed
   - All flows documented

2. **Consistency Check:**
   - Same field in different flows has consistent categorization
   - Source APIs actually exist in the specification
   - Response field names match documented schemas

3. **Clarity Check:**
   - All UNDECIDED fields have specific questions
   - All PREVIOUS_API fields reference specific source APIs
   - All categorizations have reasoning

4. **Documentation Check:**
   - All referenced APIs are documented
   - All file references are valid
   - All quotes from specs are accurate

---

## Example Workflow

### Input: Payment Connector Technical Specification

### Process:

1. **Identify Flows:**
   - Authorize
   - Capture  
   - Refund
   - Void

2. **For Authorize Flow:**

   **Fields in /v1/payments/authorize request:**
   - amount → USER_PROVIDED (transaction data)
   - currency → USER_PROVIDED (transaction data)
   - payment_method_token → PREVIOUS_API (from /v1/payment-methods/tokenize)
   - customer_id → PREVIOUS_API (from /v1/customers)
   - merchant_account_id → CONFIGURATION
   - reference → UNDECIDED (could be merchant's or from CreateOrder)

3. **Prerequisite APIs:**
   - POST /v1/auth/session → access_token
   - POST /v1/customers → customer_id
   - POST /v1/payment-methods/tokenize → payment_method_token

4. **Dependency Chain:**
   ```
   /v1/auth/session
   └─> access_token
       ├─> Used in: /v1/customers (Authorization header)
       ├─> Used in: /v1/payment-methods/tokenize (Authorization header)
       └─> Used in: /v1/payments/authorize (Authorization header)
   
   /v1/customers
   └─> customer_id
       └─> Used in: /v1/payments/authorize (request.customer_id)
   
   /v1/payment-methods/tokenize
   └─> payment_method_token
       └─> Used in: /v1/payments/authorize (request.payment_method_token)
   ```

5. **UNDECIDED Field:**
   ```
   Field: reference
   Question: Is this the merchant's internal order reference (USER_PROVIDED) 
             or does it require a CreateOrder API call (PREVIOUS_API)?
   ```

6. **Output:** Generate all analysis files with findings.

---

## Tips for Effective Analysis

1. **Start with Authentication:** Almost always needs to be done first
2. **Look for "Create" APIs:** Customer, Order, Session, Token - these often provide IDs
3. **Check Response Schemas:** The output files often show what fields are returned
4. **Follow the Money:** Payment amount/currency is almost always USER_PROVIDED
5. **IDs are Usually PREVIOUS_API:** Unless explicitly merchant-generated
6. **When in Doubt, Ask:** Better to mark UNDECIDED than guess wrong

---

## Common Patterns

### Pattern 1: OAuth Flow
```
1. POST /oauth/token → access_token
2. Use access_token in all subsequent calls
```

### Pattern 2: Customer First
```
1. POST /customers → customer_id
2. POST /payment-methods (with customer_id) → payment_method_id
3. POST /payments/authorize (with customer_id and payment_method_id)
```

### Pattern 3: Session-Based
```
1. POST /sessions → session_token
2. POST /checkout (with session_token) → checkout_id
3. POST /payments (with checkout_id)
```

### Pattern 4: Two-Step Auth
```
1. POST /payments/authorize → authorization_id
2. POST /payments/capture (with authorization_id)
```

---

## Error Handling

If during analysis:

**Error: Cannot find source API for a field**
- Mark as UNDECIDED
- Ask user: "Which API provides [field_name]?"
- Request documentation for that API

**Error: Circular dependency detected**
- Document the circular reference
- Ask user: "API A needs field from API B, but API B needs field from API A. How is this resolved?"

**Error: Conflicting information in spec**
- Document both versions
- Ask user: "Specification shows conflicting info for [field]. Which is correct?"

**Error: Missing required documentation**
- List what's missing
- Ask user: "Please provide documentation for [API_name] showing request/response schema"

---

## Final Deliverables Checklist

When completing analysis, ensure:

- [ ] All flows identified and analyzed
- [ ] All fields categorized (no missing categorizations)
- [ ] All PREVIOUS_API fields have source API identified
- [ ] All UNDECIDED fields have specific questions prepared
- [ ] Dependency chains created for all flows
- [ ] Summary statistics compiled
- [ ] All output files generated in proper structure
- [ ] Questions document ready for user review
- [ ] Cross-references validated
- [ ] No assumptions made without documentation

---

## Integration with Claude Code

This rule set is designed to be consumed by Claude Code to:

1. **Systematically analyze** technical specifications
2. **Generate comprehensive** field dependency mappings
3. **Identify gaps** in understanding automatically
4. **Prepare targeted questions** for users
5. **Create reusable documentation** for connector implementations

**Usage in Claude Code:**
```bash
# Read technical specification
# Apply this rule set step by step
# Generate all output files
# Present findings and questions to user
```

---

## Revision History

- v1.0 - Initial rule set creation
- Document will be updated based on user feedback and real-world usage

---

## Notes

- This is a **deterministic process** - same spec should yield same categorizations
- **Documentation is key** - Always reference sources for categorizations
- **User clarification is preferred** over assumptions
- **Be thorough** - Missing a dependency can break the entire flow
