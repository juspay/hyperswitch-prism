# UCS Connector Code Quality Feedback Database

---

# 📋 QUALITY REVIEW REPORT TEMPLATE

> **Instructions for Quality Guardian Subagent:**
> Use this template when conducting quality reviews after each flow implementation and for final comprehensive reviews.

---

## Quality Review Report: [ConnectorName] - [FlowName/Comprehensive]

**Review Date:** [YYYY-MM-DD]
**Reviewer:** Quality Guardian Subagent
**Phase:** Foundation | Authorize | PSync | Capture | Refund | RSync | Void | Final

---

### 🎯 Overall Quality Score: [Score]/100

```
Quality Score Calculation:
= 100 - (Critical Issues × 20) - (Warning Issues × 5) - (Suggestion Issues × 1)

Thresholds:
- 95-100: Excellent ✨ - Auto-pass
- 80-94:  Good ✅ - Pass with minor notes
- 60-79:  Fair ⚠️ - Pass with warnings
- 40-59:  Poor ❌ - Block with required fixes
- 0-39:   Critical 🚨 - Block immediately
```

**Status:** ✅ PASS | ⚠️ PASS WITH WARNINGS | ❌ BLOCKED

---

### 📊 Issue Summary

| Severity | Count | Impact on Score |
|----------|-------|-----------------|
| 🚨 Critical | [N] | -[N × 20] |
| ⚠️ Warning | [N] | -[N × 5] |
| 💡 Suggestion | [N] | -[N × 1] |

---

### 🚨 Critical Issues (Must Fix Before Proceeding) - Count: [N]

#### CRITICAL-[N]: [Issue Title]

**Feedback ID:** FB-XXX (if exists in database)
**Category:** UCS_PATTERN_VIOLATION | RUST_BEST_PRACTICE | SECURITY | etc.
**Location:** `file_path:line_number`

**Problem:**
```
[Clear description of what is wrong]
```

**Code Example:**
```rust
// Current problematic code
[code snippet]
```

**Why This Is Critical:**
[Explanation of why this must be fixed]

**Required Fix:**
```rust
// Correct implementation
[fixed code snippet]
```

**References:**
- See: guides/patterns/pattern_[flow].md
- See: feedback.md#FB-XXX
- Related: [Other feedback entries]

**Auto-Fix Available:** Yes | No
**Estimated Fix Time:** [X minutes]

---

### ⚠️ Warning Issues (Should Fix) - Count: [N]

#### WARNING-[N]: [Issue Title]

**Feedback ID:** FB-XXX (if exists in database)
**Category:** CODE_QUALITY | CONNECTOR_PATTERN | PERFORMANCE | etc.
**Location:** `file_path:line_number`

**Problem:**
[Description of the suboptimal pattern]

**Current Code:**
```rust
[code snippet]
```

**Recommended Improvement:**
```rust
[improved code snippet]
```

**Impact:**
[What improves if this is fixed]

**References:**
- See: [relevant documentation]

---

### 💡 Suggestions (Nice to Have) - Count: [N]

#### SUGGESTION-[N]: [Issue Title]

**Category:** DOCUMENTATION | TESTING_GAP | etc.
**Location:** `file_path:line_number`

**Suggestion:**
[What could be improved]

**Benefit:**
[Why this would be beneficial]

---

### ✨ Success Patterns Observed - Count: [N]

#### SUCCESS-[N]: [What Was Done Well]

**Category:** [Category]
**Location:** `file_path:line_number`

**Pattern:**
```rust
[example of good code]
```

**Why This Is Good:**
[Explanation of what makes this excellent]

**Reusability:**
[Can this pattern be applied elsewhere?]

---

### 📈 Quality Metrics

#### UCS Pattern Compliance
- [✅/❌] RouterDataV2 usage (not RouterData)
- [✅/❌] ConnectorIntegrationV2 usage (not ConnectorIntegration)
- [✅/❌] domain_types imports (not hyperswitch_domain_models)
- [✅/❌] Generic connector struct pattern `ConnectorName<T>`
- [✅/❌] Proper trait implementations

#### Code Quality
- [✅/❌] No code duplication
- [✅/❌] Proper error handling
- [✅/❌] Consistent naming conventions
- [✅/❌] Adequate documentation
- [✅/❌] Efficient transformations

#### Flow-Specific Compliance
- [✅/❌] Pattern file followed (guides/patterns/pattern_[flow].md)
- [✅/❌] All required methods implemented
- [✅/❌] Proper status mapping
- [✅/❌] Payment method handling
- [✅/❌] Edge cases considered

---

### 🎯 Decision & Next Steps

**Decision:** ✅ APPROVE TO PROCEED | ⚠️ APPROVE WITH WARNINGS | ❌ BLOCK UNTIL FIXES APPLIED

**Blocking Justification (if blocked):**
[Why this implementation cannot proceed]

**Required Actions:**
1. [Action 1 - with file and line number]
2. [Action 2 - with file and line number]
3. [Action 3 - with file and line number]

**Optional Actions (Recommended):**
1. [Improvement 1]
2. [Improvement 2]

**Estimated Total Fix Time:** [X minutes]

**Auto-Fix Commands (if available):**
```bash
# Commands to automatically fix issues
[auto-fix commands]
```

---

### 📝 Knowledge Base Updates

**New Patterns Identified:**
- [ ] Add to feedback.md: [Pattern description]
- [ ] Update frequency for: FB-XXX

**Lessons Learned:**
[Any new insights from this review]

---

### 🔄 Follow-Up Required

**If Blocked:**
- Implementer must fix critical issues
- Re-run quality review after fixes
- Confirm all critical issues resolved

**If Passed:**
- Proceed to next flow/phase
- Document success patterns
- Update metrics

---

**End of Quality Review Report**

---

---

# 🎯 PURPOSE & USAGE

## What Is This Database?

The UCS Connector Code Quality Feedback Database is a **living knowledge base** that captures:

1. **Quality Standards** - What defines excellent UCS connector code
2. **Common Issues** - Recurring problems and how to fix them
3. **Success Patterns** - Examples of exceptional implementations
4. **Anti-Patterns** - What to avoid and why
5. **Learning History** - How our understanding evolves over time

## Who Uses This?

### Primary User: Quality Guardian Subagent
- Reads this database before each quality review
- Uses the review template above for structured feedback
- Checks code against documented patterns
- Updates this database with new learnings

### Secondary Users: Developers
- Reference for understanding quality expectations
- Source of examples for correct implementations
- Guide for fixing common issues
- Documentation of tribal knowledge

## How to Use This Database

### For Quality Guardian Subagent:

1. **Before Review:**
   - Read entire feedback.md
   - Identify relevant patterns for current flow
   - Prepare checklist from applicable feedback entries

2. **During Review:**
   - Compare implementation against documented patterns
   - Check for known anti-patterns
   - Validate UCS compliance using critical patterns
   - Calculate quality score

3. **After Review:**
   - Generate report using template above
   - Add new patterns if discovered
   - Update frequency counts for existing issues
   - Document success patterns

### For Developers:

1. **Before Implementation:**
   - Review critical patterns (Section 1)
   - Read flow-specific best practices (Section 3)
   - Understand common anti-patterns to avoid (Section 5)

2. **During Implementation:**
   - Reference success patterns (Section 6)
   - Check UCS-specific guidelines (Section 2)
   - Validate payment method patterns (Section 4)

3. **After Quality Review:**
   - Read feedback carefully
   - Apply required fixes
   - Learn from suggestions
   - Ask questions if unclear

---

# 📊 FEEDBACK CATEGORIES & SEVERITY LEVELS

## Category Taxonomy

### 1. UCS_PATTERN_VIOLATION
**Focus:** UCS-specific architecture violations

**Examples:**
- Using `RouterData` instead of `RouterDataV2`
- Using `ConnectorIntegration` instead of `ConnectorIntegrationV2`
- Importing from `hyperswitch_domain_models` instead of `domain_types`
- Missing generic type parameter `<T: PaymentMethodDataTypes>`

**Severity Range:** Usually CRITICAL or WARNING

---

### 2. RUST_BEST_PRACTICE
**Focus:** Idiomatic Rust code issues

**Examples:**
- Unnecessary clones
- Inefficient iterators
- Improper error handling
- Unwrap usage where Result should propagate
- Missing trait bounds
- Non-idiomatic patterns

**Severity Range:** Usually WARNING or SUGGESTION

---

### 3. CONNECTOR_PATTERN
**Focus:** Payment connector pattern violations

**Examples:**
- Inconsistent status mapping
- Improper payment method handling
- Non-standard transformer structure
- Missing error response fields
- Incorrect authentication flow

**Severity Range:** WARNING to CRITICAL depending on impact

---

### 4. CODE_QUALITY
**Focus:** General code quality issues

**Examples:**
- Code duplication
- Poor naming conventions
- Lack of modularity
- Excessive complexity
- Missing documentation
- Inconsistent formatting

**Severity Range:** Usually WARNING or SUGGESTION

---

### 5. TESTING_GAP
**Focus:** Missing or inadequate tests

**Examples:**
- No unit tests for transformers
- Missing integration tests
- Uncovered error scenarios
- Missing edge case tests
- Insufficient test coverage

**Severity Range:** Usually WARNING or SUGGESTION

---

### 6. DOCUMENTATION
**Focus:** Documentation issues

**Examples:**
- Missing function documentation
- Unclear code comments
- Undocumented complex logic
- Missing implementation notes
- Outdated documentation

**Severity Range:** Usually SUGGESTION

---

### 7. PERFORMANCE
**Focus:** Performance anti-patterns

**Examples:**
- Inefficient data structures
- Unnecessary allocations
- Repeated computations
- Inefficient transformations
- Missing memoization opportunities

**Severity Range:** Usually WARNING or SUGGESTION

---

### 8. SECURITY
**Focus:** Security concerns

**Examples:**
- Exposed sensitive data
- Missing input validation
- Unsafe operations
- Improper credential handling
- Missing sanitization

**Severity Range:** Usually CRITICAL

---

### 9. SUCCESS_PATTERN
**Focus:** What worked well (positive reinforcement)

**Examples:**
- Excellent error handling
- Reusable transformer logic
- Clean separation of concerns
- Comprehensive test coverage
- Well-documented complex logic

**Severity Range:** INFO (positive feedback)

---

## Severity Levels

### 🚨 CRITICAL
**Definition:** Must be fixed immediately, blocks progression

**Criteria:**
- Breaks UCS architectural conventions
- Security vulnerabilities
- Will cause runtime failures
- Violates core requirements
- Makes code unmaintainable

**Score Impact:** -20 points per issue

**Examples:**
- Using wrong UCS types (RouterData vs RouterDataV2)
- Missing mandatory trait implementations
- Exposed API keys or credentials
- Broken error handling

---

### ⚠️ WARNING
**Definition:** Should be fixed, but not blocking

**Criteria:**
- Suboptimal but functional
- Technical debt accumulation
- Maintenance concerns
- Performance issues
- Inconsistent patterns

**Score Impact:** -5 points per issue

**Examples:**
- Code duplication
- Non-idiomatic Rust
- Missing test coverage
- Inefficient transformations

---

### 💡 SUGGESTION
**Definition:** Nice-to-have improvements

**Criteria:**
- Enhancement opportunities
- Code quality improvements
- Documentation additions
- Refactoring opportunities
- Learning opportunities

**Score Impact:** -1 point per issue

**Examples:**
- Better variable names
- Additional comments
- Extracted helper functions
- More comprehensive tests

---

### ✨ INFO
**Definition:** Positive feedback, no score impact

**Criteria:**
- Exemplary implementations
- Reusable patterns
- Excellent practices
- Learning examples
- Success stories

**Score Impact:** 0 (positive reinforcement only)

**Examples:**
- Clean, reusable code
- Comprehensive error handling
- Well-structured transformers
- Excellent test coverage

---

# 📝 FEEDBACK ENTRY TEMPLATE

## How to Add New Feedback

When you discover a new pattern, issue, or best practice, add it to the appropriate section using this template:

```markdown
### FB-[ID]: [Brief Descriptive Title]

**Metadata:**
```yaml
id: FB-XXX
category: [CATEGORY_NAME]
severity: CRITICAL | WARNING | SUGGESTION | INFO
connector: [connector_name] | general
flow: [Authorize|Capture|Void|Refund|PSync|RSync] | All
date_added: YYYY-MM-DD
status: Active | Resolved | Archived
frequency: [number] # How many times observed
impact: High | Medium | Low
tags: [tag1, tag2, tag3]
```

**Issue Description:**
[Clear, concise description of what the issue is or what pattern to follow]

**Context / When This Applies:**
[Explain when this issue typically occurs or when this pattern should be used]

**Code Example - WRONG (if applicable):**
```rust
// Example of incorrect implementation
[problematic code snippet]
```

**Code Example - CORRECT:**
```rust
// Example of correct implementation
[correct code snippet]
```

**Why This Matters:**
[Explain the impact - why is this important?]

**How to Fix:**
1. [Step-by-step fix instructions]
2. [Include file locations and specific changes]
3. [Provide reasoning for each step]

**Auto-Fix Rule (if applicable):**
```
IF [condition]
THEN [action]
EXAMPLE: IF file contains "RouterData<" AND NOT "RouterDataV2<"
THEN suggest: "Replace RouterData with RouterDataV2"
```

**Related Patterns:**
- See: guides/patterns/pattern_[name].md#section
- See: FB-XXX (related feedback entry)
- Reference: [external documentation link]

**Lessons Learned:**
[Key takeaways, gotchas, or insights]

**Prevention:**
[How to avoid this issue in future implementations]

---
```

## Feedback ID Numbering Convention

The feedback database uses semantic category-based prefixes for feedback IDs, allowing unlimited entries per category:

### Active ID Prefixes

- **UCS-XXX:** UCS-Specific Architectural Guidelines
  - Example: UCS-001, UCS-002, UCS-003...
  - Use for: UCS architecture patterns, RouterDataV2, ConnectorIntegrationV2, domain_types

- **ANTI-XXX:** Common Anti-Patterns to Avoid
  - Example: ANTI-001, ANTI-002, ANTI-003...
  - Use for: What NOT to do, common mistakes, problematic patterns

- **SEC-XXX:** Security Guidelines and Patterns
  - Example: SEC-001, SEC-002, SEC-003...
  - Use for: Security concerns, unsafe code, credential handling

- **FLOW-XXX:** Flow-Specific Best Practices
  - Example: FLOW-001, FLOW-002, FLOW-003...
  - Use for: Patterns specific to Authorize, Capture, Void, Refund, PSync, RSync

- **METHOD-XXX:** Payment Method Patterns
  - Example: METHOD-001, METHOD-002, METHOD-003...
  - Use for: Card, wallet, bank transfer, BNPL patterns

- **SUCCESS-XXX:** Success Patterns and Examples
  - Example: SUCCESS-001, SUCCESS-002, SUCCESS-003...
  - Use for: Exemplary implementations worth celebrating

- **PERF-XXX:** Performance Patterns and Optimizations
  - Example: PERF-001, PERF-002, PERF-003...
  - Use for: Performance anti-patterns, optimizations

- **TEST-XXX:** Testing Patterns and Gaps
  - Example: TEST-001, TEST-002, TEST-003...
  - Use for: Test coverage, testing strategies

- **DOC-XXX:** Documentation Patterns
  - Example: DOC-001, DOC-002, DOC-003...
  - Use for: Documentation standards, clarity improvements

### Legacy FB-XXX Range (Deprecated)

The old FB-XXX numbering system (FB-001 to FB-999) has been replaced by semantic prefixes. All new feedback entries should use the category-based prefixes above. The FB-XXX range is maintained for the example in Section 1 only.

---

---

# 1. CRITICAL PATTERNS (MUST FOLLOW)

> **Purpose:** Non-negotiable UCS architectural requirements that MUST be followed in every connector implementation.

**Status:** Ready for population - Add critical patterns discovered during connector implementations

**Template Example:**

### FB-001: Use RouterDataV2, Never RouterData

**Metadata:**
```yaml
id: FB-001
category: UCS_PATTERN_VIOLATION
severity: CRITICAL
connector: general
flow: All
date_added: 2024-01-15
status: Active
frequency: 0
impact: High
tags: [ucs-architecture, router-data, breaking-change]
```

**Issue Description:**
UCS architecture requires `RouterDataV2` instead of legacy `RouterData`. Using the wrong type will cause compilation failures and architectural incompatibilities.

**Code Example - WRONG:**
```rust
use hyperswitch_domain_models::router_data::RouterData;

fn process_payment(
    data: &RouterData<Flow, Request, Response>
) -> Result<...> {
    // This will not compile in UCS
}
```

**Code Example - CORRECT:**
```rust
use domain_types::router_data_v2::RouterDataV2;

fn process_payment(
    data: &RouterDataV2<Flow, FlowData, Request, Response>
) -> Result<...> {
    // Correct UCS pattern
}
```

**Why This Matters:**
- UCS uses enhanced type safety with separate flow data
- RouterDataV2 provides better separation of concerns
- Required for gRPC integration
- Ensures compatibility with UCS architecture

**How to Fix:**
1. Find all occurrences of `RouterData<`
2. Replace with `RouterDataV2<`
3. Add appropriate flow data type parameter
4. Update imports to `domain_types::router_data_v2::RouterDataV2`

**Auto-Fix Rule:**
```
IF file contains "RouterData<" AND NOT "RouterDataV2<"
THEN suggest: "Replace RouterData with RouterDataV2 and add flow data parameter"
```

**Related Patterns:**
- See: guides/patterns/README.md#ucs-architecture
- See: FB-002 (ConnectorIntegrationV2)

**Prevention:**
- Always use UCS templates as starting point
- Run quality checks after each flow implementation
- Reference existing UCS connectors for patterns

---

**[More critical patterns will be added here as they are discovered]**

---

---

# 2. UCS-SPECIFIC GUIDELINES

> **Purpose:** UCS architectural patterns and conventions specific to the connector-service implementation.

**Status:** Active - Contains 4 critical UCS-specific guidelines

---

### UCS-001: Use amount conversion framework instead of manual conversion

**Metadata:**
```yaml
id: UCS-001
category: UCS_PATTERN_VIOLATION
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [ucs-architecture, amount-conversion, framework, critical]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Manually converting amounts without using the UCS amount conversion framework violates UCS architectural standards. All amount conversions must use the standardized amount conversion framework declared in the `amount_converters` field of the `create_all_prerequisites` macro.

**Context / When This Applies:**
This applies to all payment flows (Authorize, Capture, Refund) that need to convert amounts between different representations (e.g., from major currency units to minor units, handling currency-specific conversions). The framework must be used instead of implementing manual conversion logic.

**Code Example - WRONG:**
```rust
// Missing amount converters - manual conversion happening elsewhere
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    api: [
        (
            flow: Authorize,
            request_body: {{ConnectorName}}AuthorizeRequest<T>,
        ),
    ],
    amount_converters: [],  // Empty - wrong!
)
```

**Code Example - CORRECT:**
```rust
// Proper amount converter declaration
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    api: [
        (
            flow: Authorize,
            request_body: {{ConnectorName}}AuthorizeRequest<T>,
        ),
    ],
    amount_converters: [
        (flow: Authorize, converter: AuthorizeAmountConverter),
        (flow: Capture, converter: CaptureAmountConverter),
        (flow: Refund, converter: RefundAmountConverter),
    ],
)
```

**Why This Matters:**
UCS provides a standardized amount conversion framework that handles currency conversions, minor unit calculations, and amount validation consistently. Using this framework ensures:
- Correctness: Proper handling of different currency minor units (cents, paise, etc.)
- Consistency: All connectors handle amounts the same way
- Type Safety: Compile-time validation of amount conversions
- Maintainability: Centralized logic for amount handling

**How to Fix:**
1. Locate the `create_all_prerequisites!` macro in your connector's `mod.rs` file
2. Add an `amount_converters` field if it doesn't exist
3. For each payment flow (Authorize, Capture, Refund), declare the appropriate converter:
   - Create converter structs implementing the amount conversion trait
   - Register them in the `amount_converters` array
4. Remove any manual amount conversion logic from transformers
5. Use the `MinorUnit` type for all amount fields (see UCS-002)

**Auto-Fix Rule:**
```
IF create_all_prerequisites macro contains "amount_converters: []"
THEN suggest: "Add amount converters for all payment flows: Authorize, Capture, Refund"
```

**Related Patterns:**
- See: UCS-002 (Use MinorUnit type for amounts)
- See: guides/patterns/pattern_authorize.md#amount-handling
- Reference: domain_types documentation on amount conversion

**Lessons Learned:**
- The UCS amount conversion framework is mandatory, not optional
- Even if a connector uses the same amount representation as UCS, converters should still be declared for consistency
- Missing amount converters often leads to runtime errors during payment processing

**Prevention:**
- Always start with the UCS connector template which includes amount converter declarations
- Review the `create_all_prerequisites` macro during foundation setup
- Run quality checks after foundation phase to ensure converters are present

---

### UCS-002: Use MinorUnit type for amount fields instead of primitive types

**Metadata:**
```yaml
id: UCS-002
category: UCS_PATTERN_VIOLATION
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [ucs-architecture, types, amounts, minor-unit, critical]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Using primitive types like `i64`, `u64`, or `f64` for amount fields in request/response structures violates UCS type safety standards. All amount fields must use the `domain_types::MinorUnit` type.

**Context / When This Applies:**
This applies to all struct definitions for payment requests and responses where monetary amounts are represented. Any field representing an amount (payment amount, refund amount, fee amount, etc.) must use `MinorUnit`.

**Code Example - WRONG:**
```rust
// Using primitive types for amounts
pub struct {{ConnectorName}}PaymentRequest {
    pub amount: i64,          // Wrong!
    pub fee: u64,             // Wrong!
    pub total: f64,           // Wrong!
    pub currency: String,
}
```

**Code Example - CORRECT:**
```rust
use domain_types::MinorUnit;
use common_enums::Currency;

pub struct {{ConnectorName}}PaymentRequest {
    pub amount: MinorUnit,     // Correct!
    pub fee: MinorUnit,        // Correct!
    pub total: MinorUnit,      // Correct!
    pub currency: Currency,    // Also use proper Currency enum
}
```

**Why This Matters:**
The `MinorUnit` type ensures:
- Type Safety: Prevents mixing of amounts in different representations (major vs minor units)
- Precision: Avoids floating-point precision issues with monetary calculations
- Consistency: All connectors represent amounts the same way
- Currency Awareness: Works with the amount conversion framework to handle currency-specific minor units
- Compile-Time Validation: Type system enforces correct usage

**How to Fix:**
1. Import `domain_types::MinorUnit` at the top of your requests/responses file
2. Find all fields with types `i64`, `u64`, `i32`, `u32`, or `f64` that represent amounts
3. Replace the primitive type with `MinorUnit`
4. Update any serialization/deserialization logic to handle `MinorUnit`
5. Ensure amount converters are properly declared (see UCS-001)
6. Update transformers to work with `MinorUnit` instead of raw numbers

**Auto-Fix Rule:**
```
IF struct field name matches "(amount|fee|total|price|charge)" AND type is NOT MinorUnit
THEN suggest: "Replace primitive type with domain_types::MinorUnit"
```

**Related Patterns:**
- See: UCS-001 (Amount conversion framework)
- See: guides/patterns/README.md#domain-types
- Reference: domain_types::MinorUnit documentation

**Lessons Learned:**
- Never use `f64` for monetary amounts - it causes precision loss
- Even if a connector API accepts integers, use `MinorUnit` internally and convert during serialization
- `MinorUnit` provides helper methods for common operations

**Prevention:**
- Always use UCS struct templates as starting points
- Add `MinorUnit` to import checklist during foundation phase
- Run type checks during quality review to catch primitive amount types
- Reference existing UCS connectors for amount field patterns

---

### UCS-003: Separate 3DS authentication into Pre-Authenticate and Post-Authenticate flows

**Metadata:**
```yaml
id: UCS-003
category: CONNECTOR_PATTERN
severity: CRITICAL
connector: general
flow: Authenticate
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [3ds, authentication, flow-separation, state-machine]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Handling all 3DS authentication logic (both device data collection and challenge response) in a single `Authenticate` flow violates the UCS state machine pattern for 3DS. The two distinct phases must be modeled as separate flows: `PreAuthenticate` for device data collection and `PostAuthenticate` for challenge handling.

**Context / When This Applies:**
This applies when implementing 3DS (3D Secure) authentication support for connectors. The connector may call it "authentication" generically, but UCS requires separating the two phases:
- PreAuthenticate: Initiating 3DS, collecting device data (3dsDeviceData), fingerprinting
- PostAuthenticate: Handling the challenge response (3dsChallenges) after user authentication

**Code Example - WRONG:**
```rust
// Single Authenticate flow - wrong!
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    api: [
        (
            flow: Authenticate,
            request_body: {{ConnectorName}}AuthenticateRequest,
            response_body: {{ConnectorName}}AuthenticateResponse,
        ),
    ],
)
```

**Code Example - CORRECT:**
```rust
// Separate Pre-Authenticate and Post-Authenticate flows
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    api: [
        // Handle 3dsDeviceData in Pre-Authenticate
        (
            flow: PreAuthenticate,
            request_body: {{ConnectorName}}PreAuthenticateRequest,
            response_body: {{ConnectorName}}PreAuthenticateResponse,
        ),
        // Handle 3dsChallenges in Post-Authenticate
        (
            flow: PostAuthenticate,
            request_body: {{ConnectorName}}PostAuthenticateRequest,
            response_body: {{ConnectorName}}PostAuthenticateResponse,
        ),
    ],
)
```

**Why This Matters:**
3DS authentication has two distinct phases with different:
- Request/Response Structures: Device data collection vs challenge response
- State Transitions: Initial authentication vs challenge completion
- Error Handling: Different failure modes for each phase
- Data Requirements: Different inputs required for each step

Modeling them as separate flows ensures:
- Correct state machine transitions
- Proper handling of each phase's unique requirements
- Clear separation of concerns
- Better error handling and debugging

**How to Fix:**
1. Locate your `Authenticate` flow in `create_all_prerequisites` macro
2. Replace it with two separate flows: `PreAuthenticate` and `PostAuthenticate`
3. Create separate request/response structs for each flow:
   - `{{ConnectorName}}PreAuthenticateRequest` - handles device data
   - `{{ConnectorName}}PostAuthenticateRequest` - handles challenge response
4. Implement separate transformers for each flow
5. In `PreAuthenticate` transformer, handle `3dsDeviceData` from router_data
6. In `PostAuthenticate` transformer, handle `3dsChallenges` from router_data
7. Update status mapping to reflect the two-phase authentication

**Auto-Fix Rule:**
```
IF flow contains single "Authenticate" AND connector supports 3DS
THEN suggest: "Split into PreAuthenticate (device data) and PostAuthenticate (challenge) flows"
```

**Related Patterns:**
- See: FB-200 series (Flow-Specific Best Practices when populated)
- See: guides/patterns/pattern_authenticate.md
- Reference: UCS 3DS state machine documentation

**Lessons Learned:**
- Even if the connector API has a single "authenticate" endpoint, UCS requires modeling as two flows
- The separation improves testability - you can test each phase independently
- Error handling is clearer when each phase has its own flow

**Prevention:**
- Review connector API documentation for 3DS support early
- Plan for Pre/Post authenticate separation during design phase
- Reference existing UCS connectors with 3DS support for patterns
- Include both flows in the initial macro setup

---

### UCS-004: Use flow-specific request types in macro configuration

**Metadata:**
```yaml
id: UCS-004
category: CONNECTOR_PATTERN
severity: WARNING
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: Medium
tags: [naming-conventions, type-safety, code-clarity]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Using generic request type names like `PaymentsRequest` for specific flows (Authorize, Capture, Void, etc.) in the `create_all_prerequisites` macro reduces code clarity and type safety. Request types should be named after the specific flow they serve.

**Context / When This Applies:**
This applies when defining the `request_body` types in the `create_all_prerequisites` macro. Each flow should have its own specifically-named request type rather than generic names like `PaymentsRequest` or `PaymentRequest`.

**Code Example - WRONG:**
```rust
// Using generic "PaymentsRequest" for Authorize flow
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    api: [
        (
            flow: Authorize,
            request_body: {{ConnectorName}}PaymentsRequest<T>,  // Too generic!
        ),
        (
            flow: Capture,
            request_body: {{ConnectorName}}PaymentsRequest<T>,  // Same type for different flow!
        ),
    ],
)
```

**Code Example - CORRECT:**
```rust
// Using flow-specific request types
macros::create_all_prerequisites!(
    connector_name: {{ConnectorName}},
    api: [
        (
            flow: Authorize,
            request_body: {{ConnectorName}}AuthorizeRequest<T>,  // Clear and specific
        ),
        (
            flow: Capture,
            request_body: {{ConnectorName}}CaptureRequest<T>,    // Clear and specific
        ),
        (
            flow: Void,
            request_body: {{ConnectorName}}VoidRequest<T>,       // Clear and specific
        ),
        (
            flow: Refund,
            request_body: {{ConnectorName}}RefundRequest<T>,     // Clear and specific
        ),
    ],
)
```

**Why This Matters:**
Flow-specific naming provides:
- Code Clarity: Immediately obvious which request structure is used for which flow
- Type Safety: Each flow has its own type, preventing accidental misuse
- Maintainability: Changes to one flow's request don't affect others
- Documentation: The code is self-documenting
- IDE Support: Better autocomplete and type hints

**How to Fix:**
1. Locate the `create_all_prerequisites!` macro in your connector's `mod.rs`
2. For each flow entry, check the `request_body` type name
3. If it uses generic naming like `PaymentsRequest`, rename it:
   - Authorize → `{{ConnectorName}}AuthorizeRequest<T>`
   - Capture → `{{ConnectorName}}CaptureRequest<T>`
   - Void → `{{ConnectorName}}VoidRequest<T>`
   - Refund → `{{ConnectorName}}RefundRequest<T>`
   - PSync → `{{ConnectorName}}PSyncRequest<T>`
   - RSync → `{{ConnectorName}}RSyncRequest<T>`
4. Update the corresponding struct definitions in `requests.rs`
5. Update transformers to use the renamed types
6. Do the same for `response_body` types

**Auto-Fix Rule:**
```
IF flow is "Authorize" AND request_body contains "PaymentsRequest"
THEN suggest: "Rename to {{ConnectorName}}AuthorizeRequest<T>"
APPLY similar rule for: Capture, Void, Refund, PSync, RSync
```

**Related Patterns:**
- See: Rust naming conventions
- See: guides/patterns/README.md#naming-conventions
- Reference: UCS connector structure guidelines

**Lessons Learned:**
- Specific names are better than generic names, even if types are currently identical
- Even if multiple flows use the same structure now, they may diverge in the future
- Flow-specific naming makes code reviews easier

**Prevention:**
- Use flow-specific names from the start when creating request/response structs
- Review macro configuration during foundation phase
- Check that each flow has its own request/response types
- Reference existing UCS connectors for naming patterns

---

---

---

# 3. FLOW-SPECIFIC BEST PRACTICES

> **Purpose:** Best practices specific to each payment flow (Authorize, Capture, Void, Refund, PSync, RSync)

**Status:** Ready for population - Add flow-specific patterns as connectors are implemented

**Organization:**
Organize by flow:
- Authorize Flow Patterns
- Capture Flow Patterns
- Void Flow Patterns
- Refund Flow Patterns
- PSync Flow Patterns
- RSync Flow Patterns

**Guidance:**
- Document flow-specific transformer patterns
- Capture status mapping strategies
- Note common flow-specific errors
- Record successful implementations

**[Content will be added here based on implementation learnings]**

---

---

# 4. PAYMENT METHOD PATTERNS

> **Purpose:** Best practices for implementing different payment methods (cards, wallets, bank transfers, etc.)

**Status:** Ready for population - Add payment method patterns as they are discovered

**Organization:**
Organize by payment method:
- Card Payment Patterns
- Wallet Payment Patterns (Apple Pay, Google Pay, etc.)
- Bank Transfer Patterns
- BNPL Patterns
- Regional Payment Method Patterns

**Guidance:**
- Document payment method transformations
- Capture validation requirements
- Note payment method specific edge cases
- Record successful implementations

**[Content will be added here based on implementation learnings]**

---

---

# 5. COMMON ANTI-PATTERNS

> **Purpose:** Document what NOT to do - common mistakes and anti-patterns to avoid

**Status:** Active - Contains 11 common anti-patterns to avoid

---

## Code Quality Anti-Patterns (CRITICAL)

### ANTI-001: Avoid hardcoding values - use constants or extract from data

**Metadata:**
```yaml
id: ANTI-001
category: CODE_QUALITY
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [hardcoding, maintainability, configuration, anti-pattern]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Hardcoding string literals, URLs, API versions, reference IDs, or business logic values directly in code makes it unmaintainable, untestable, and error-prone. All static values should be declared as named constants, and dynamic values should be extracted from router_data or response objects.

**Context / When This Applies:**
This applies anywhere values are used in connector code:
- API endpoints and URLs
- API versions
- Fixed string values (payment types, method names, etc.)
- Reference IDs and transaction identifiers (see ANTI-002)
- Configuration values

**Code Example - WRONG:**
```rust
// Hardcoded values scattered throughout code
let url = "https://api.example.com/v1/payments";  // Wrong!
let version = "2024-06-01";                        // Wrong!
let payment_type = "card";                         // Wrong!
let reference = "hardcoded-ref-123";               // Very wrong!

{{ConnectorName}}Request {
    api_version: "2024-06-01",     // Duplicated!
    endpoint: "/v1/payments",       // Hardcoded!
    reference_id: "test-ref",       // Dangerous!
}
```

**Code Example - CORRECT:**
```rust
// Constants declared at module level
const API_VERSION: &str = "2024-06-01";
const PAYMENTS_ENDPOINT: &str = "api/payments";
const PAYMENT_TYPE_CARD: &str = "card";

// Dynamic values extracted from data
impl From<&RouterDataV2<...>> for {{ConnectorName}}Request {
    fn from(router_data: &RouterDataV2<...>) -> Self {
        let base_url = &router_data.connector_api_config.base_url;
        let url = format!("{}{}", base_url, PAYMENTS_ENDPOINT);

        {{ConnectorName}}Request {
            api_version: API_VERSION,
            url,
            reference_id: router_data.connector_request_reference_id.clone(),
        }
    }
}
```

**Why This Matters:**
Hardcoded values cause multiple issues:
- Maintainability: Changes require finding and updating every occurrence
- Testability: Difficult to test with different values
- Consistency: Easy to have mismatched values across files
- Configuration: Cannot be configured per environment
- Errors: Typos and inconsistencies are common
- Reference IDs: Hardcoded IDs break transaction tracking and idempotency

**How to Fix:**
1. Scan code for string literals and magic numbers
2. For static values:
   - Declare as `const` at module level in `requests.rs`
   - Use descriptive names (API_VERSION, ENDPOINT_PAYMENTS, etc.)
   - Make them `pub` if used across modules
3. For dynamic values:
   - Extract from `router_data` (reference IDs, transaction IDs, amounts)
   - Extract from connector response (connector transaction IDs, etc.)
   - Never create or mutate these values
4. Import constants where needed rather than duplicating
5. Document what each constant represents

**Auto-Fix Rule:**
```
IF string literal found AND looks like API endpoint/version/config value
THEN suggest: "Extract to named constant"

IF string literal looks like reference/transaction ID
THEN error: "Never hardcode reference IDs - extract from router_data"
```

**Related Patterns:**
- See: ANTI-002 (Use reference IDs from router data)
- See: ANTI-003 (Never mutate reference IDs)
- See: ANTI-004 (Reuse existing constants)
- Reference: Rust const guidelines

**Lessons Learned:**
- Even "temporary" hardcoded values tend to become permanent
- Hardcoded reference IDs are a critical error that breaks payment processing
- Constants at module level are better than inline literals

**Prevention:**
- Set up constants file/section during foundation phase
- Code review checklist: "No magic strings or numbers"
- Use linter rules to detect hardcoded URLs and versions
- Always extract reference IDs from router_data - never create them

---

### ANTI-002: Use reference IDs from router data, never hardcode them

**Metadata:**
```yaml
id: ANTI-002
category: CODE_QUALITY
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [reference-ids, transaction-ids, idempotency, critical]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Hardcoding reference IDs or transaction identifiers in request construction breaks transaction tracking, idempotency, reconciliation, and payment processing. All reference IDs must be extracted from `router_data` or connector responses.

**Context / When This Applies:**
This applies to all fields that represent transaction identifiers:
- `reference_id`, `connector_request_reference_id`
- `transaction_id`, `connector_transaction_id`
- `payment_id`, `attempt_id`
- Any ID used for tracking or correlation

**Code Example - WRONG:**
```rust
// Hardcoding reference IDs - NEVER DO THIS!
{{ConnectorName}}Request {
    reference_id: "hardcoded-ref-123".to_string(),      // Critical error!
    transaction_id: "test-txn-456".to_string(),         // Critical error!
    merchant_reference: "test-merchant-ref".to_string(), // Critical error!
}

// Or generating your own IDs
{{ConnectorName}}Request {
    reference_id: Uuid::new_v4().to_string(),  // Wrong - don't generate!
}
```

**Code Example - CORRECT:**
```rust
// Extract from router_data
impl TryFrom<&RouterDataV2<...>> for {{ConnectorName}}Request {
    fn try_from(router_data: &RouterDataV2<...>) -> Result<Self> {
        Ok({{ConnectorName}}Request {
            reference_id: router_data.connector_request_reference_id.clone(),
            transaction_id: router_data.request.connector_transaction_id
                .clone()
                .ok_or(errors::ConnectorError::MissingRequiredField {
                    field_name: "connector_transaction_id",
                })?,
        })
    }
}

// Or extract from connector response
impl TryFrom<&{{ConnectorName}}Response> for RouterDataV2<...> {
    fn try_from(response: &{{ConnectorName}}Response) -> Result<Self> {
        Ok(Self {
            reference_id: response.transaction_reference.clone(),
            connector_transaction_id: response.id.clone(),
            // ...
        })
    }
}
```

**Why This Matters:**
Reference IDs are critical for:
- Idempotency: Ensuring requests aren't duplicated
- Transaction Tracking: Following payment through its lifecycle
- Reconciliation: Matching payments with connector records
- Debugging: Tracing issues across systems
- Compliance: Audit trails for regulatory requirements

Hardcoding or generating reference IDs causes:
- Failed idempotency checks
- Lost transaction tracking
- Reconciliation failures
- Payment processing errors
- Customer support issues

**How to Fix:**
1. Find all struct fields with names containing: `reference`, `transaction`, `id`, `attempt`
2. Verify each is populated from `router_data` or connector response
3. Remove any hardcoded values or UUID generation
4. For required fields, use `ok_or()` to return proper errors if missing:
   ```rust
   .ok_or(errors::ConnectorError::MissingRequiredField {
       field_name: "connector_transaction_id"
   })?
   ```
5. Never apply transformations to IDs (see ANTI-003)

**Auto-Fix Rule:**
```
IF field name contains ("reference"|"transaction"|"_id") AND value is hardcoded string
THEN error: "Critical - Never hardcode reference IDs. Extract from router_data or response"

IF code generates UUID for reference/transaction field
THEN error: "Don't generate IDs - use values from router_data"
```

**Related Patterns:**
- See: ANTI-001 (Avoid hardcoding values)
- See: ANTI-003 (Never mutate reference IDs)
- Reference: UCS idempotency documentation

**Lessons Learned:**
- Reference IDs are opaque identifiers - treat them as sacred
- Even in testing, use proper test data rather than hardcoded IDs
- Missing reference IDs should error early, not use defaults

**Prevention:**
- Never use placeholder values like "test-ref" even temporarily
- Code review must verify all reference IDs come from router_data
- Add validation that reference ID fields are never hardcoded
- Test with real payment flow data, not mocked IDs

---

### ANTI-003: Never mutate reference IDs or transaction identifiers

**Metadata:**
```yaml
id: ANTI-003
category: CODE_QUALITY
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [reference-ids, immutability, data-integrity, critical]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Mutating reference IDs or transaction identifiers (changing case, replacing characters, trimming, etc.) breaks idempotency, transaction tracking, reconciliation, and connector API calls. These values must be preserved exactly as received.

**Context / When This Applies:**
This applies to ANY operation on reference or transaction IDs:
- String transformations: `to_uppercase()`, `to_lowercase()`, `replace()`, `trim()`
- Format changes: Adding prefixes/suffixes, padding, truncating
- Encoding changes: URL encoding, base64, hex conversion
- Any mutation whatsoever

**Code Example - WRONG:**
```rust
// Mutating reference IDs - NEVER DO THIS!
let modified_ref = connector_ref.replace("-", "_");        // Wrong!
let mutated_id = transaction_id.to_uppercase();            // Wrong!
let cleaned = reference_id.trim();                         // Wrong!
let prefixed = format!("TXN-{}", transaction_id);         // Wrong!
let truncated = reference_id[..10].to_string();           // Wrong!

{{ConnectorName}}Request {
    reference_id: reference_id.to_lowercase(),  // Breaks everything!
}
```

**Code Example - CORRECT:**
```rust
// Use reference IDs exactly as received - NO mutations
impl TryFrom<&RouterDataV2<...>> for {{ConnectorName}}Request {
    fn try_from(router_data: &RouterDataV2<...>) -> Result<Self> {
        Ok({{ConnectorName}}Request {
            // Clone exactly as-is, no transformations
            reference_id: router_data.connector_request_reference_id.clone(),
            transaction_id: router_data.request.connector_transaction_id.clone(),
        })
    }
}

// From connector response - preserve exactly
impl TryFrom<&{{ConnectorName}}Response> for ... {
    fn try_from(response: &{{ConnectorName}}Response) -> Result<Self> {
        Ok(Self {
            // No mutations - use exactly as connector provides
            connector_transaction_id: response.transaction_id.clone(),
            reference_id: response.reference.clone(),
        })
    }
}
```

**Why This Matters:**
Reference IDs are opaque identifiers provided by payment connectors or the payment system. Mutating them causes:
- Idempotency Failures: System won't recognize requests as duplicates
- Transaction Tracking Broken: Can't find payments in logs/database
- Reconciliation Failures: Can't match payments with connector records
- API Call Failures: Connector won't recognize mutated IDs
- Data Corruption: Lost connection between related operations
- Customer Impact: Duplicate charges, failed refunds, lost payments

**How to Fix:**
1. Search code for reference/transaction ID fields
2. Trace how each ID is used from receipt to usage
3. Remove ANY string transformations:
   - Delete `.to_uppercase()`, `.to_lowercase()`
   - Delete `.replace()`, `.trim()`, `.trim_start()`, `.trim_end()`
   - Delete format strings that modify the ID
   - Delete truncation or padding operations
4. Use `.clone()` to copy IDs without modification
5. If connector requires specific format, that's a connector issue - document and report it

**Auto-Fix Rule:**
```
IF reference_id_field.contains("to_uppercase"|"to_lowercase"|"replace"|"trim")
THEN error: "Critical - Never mutate reference IDs. Use exactly as received."

IF format!() or similar wraps reference/transaction ID with prefix/suffix
THEN error: "Don't modify reference IDs - use exactly as provided"
```

**Related Patterns:**
- See: ANTI-002 (Use reference IDs from router data)
- See: ANTI-001 (Avoid hardcoding values)
- Reference: Payment system idempotency guarantees

**Lessons Learned:**
- Reference IDs are opaque - their internal format doesn't matter to connector code
- Even "cosmetic" changes like trimming whitespace can break tracking
- If a connector returns IDs with "weird" formatting, preserve it exactly
- The same ID may be used across multiple systems - any mutation breaks the chain

**Prevention:**
- Code review checklist: "Are reference IDs used exactly as received?"
- Add linter rule to flag string transformations on ID fields
- Test idempotency with exact ID matching
- Document that IDs are immutable opaque values

---

## Code Quality Anti-Patterns (WARNING)

### ANTI-004: Reuse existing constants instead of redeclaring them

**Metadata:**
```yaml
id: ANTI-004
category: CODE_QUALITY
severity: WARNING
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: Medium
tags: [code-duplication, constants, dry-principle, maintainability]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Declaring the same constants in multiple files (e.g., both `requests.rs` and `transformers.rs`) violates the DRY (Don't Repeat Yourself) principle and can lead to inconsistencies when values need to change.

**Context / When This Applies:**
This applies when you need the same constant value in multiple modules:
- API versions
- Endpoint paths
- Payment type strings
- Fixed configuration values
- Any value used in more than one place

**Code Example - WRONG:**
```rust
// In transformers.rs
const API_VERSION: &str = "2024-06-01";
const PAYMENT_TYPE: &str = "card";
const ENDPOINT_AUTHORIZE: &str = "/api/v1/authorize";

// In requests.rs (duplicate declarations!)
const API_VERSION: &str = "2024-06-01";
const PAYMENT_TYPE: &str = "card";
const ENDPOINT_AUTHORIZE: &str = "/api/v1/authorize";

// In responses.rs (even more duplicates!)
const API_VERSION: &str = "2024-06-01";
```

**Code Example - CORRECT:**
```rust
// In requests.rs (canonical location for constants)
pub const API_VERSION: &str = "2024-06-01";
pub const PAYMENT_TYPE: &str = "card";
pub const ENDPOINT_AUTHORIZE: &str = "/api/v1/authorize";
pub const ENDPOINT_CAPTURE: &str = "/api/v1/capture";

// In transformers.rs (import and use)
use super::requests::{API_VERSION, PAYMENT_TYPE, ENDPOINT_AUTHORIZE};

// In responses.rs (import and use)
use super::requests::API_VERSION;
```

**Why This Matters:**
- DRY Principle: Single source of truth for each constant
- Maintainability: Update value in one place, not scattered across files
- Consistency: Impossible to have mismatched values
- Refactoring: Easier to find and update all usages
- Code Review: Clear where constants are defined

**How to Fix:**
1. Search for duplicate `const` declarations across module files
2. Choose canonical location (usually `requests.rs` for API-related constants)
3. Keep one declaration, make it `pub`
4. In other files, add import statement
5. Remove duplicate declarations
6. Verify code still compiles and tests pass

**Auto-Fix Rule:**
```
IF const declaration found in multiple files with same value
THEN suggest: "Declare in requests.rs as pub, import in other files"
```

**Related Patterns:**
- See: ANTI-001 (Avoid hardcoding values)
- See: ANTI-005 (Remove duplicate functions)
- Reference: DRY principle

**Lessons Learned:**
- Declare constants as `pub` from the start if they might be reused
- Group related constants together in the same module
- Use clear module organization to make constants easy to find

**Prevention:**
- Before declaring a const, check if it exists elsewhere
- Use IDE "find in project" for constant values
- Code review should catch duplicate declarations
- Consider a `constants.rs` module if you have many shared constants

---

### ANTI-005: Remove duplicate function implementations across modules

**Metadata:**
```yaml
id: ANTI-005
category: CODE_QUALITY
severity: WARNING
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: Medium
tags: [code-duplication, functions, dry-principle, maintainability]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Having the same function implemented in multiple files (e.g., both `transformers.rs` and `responses.rs`) violates DRY principle and creates maintenance issues. When bugs are fixed in one location, they may persist in duplicates.

**Context / When This Applies:**
This applies to any utility or helper functions that are duplicated:
- Status parsing functions
- Data validation functions
- Format conversion functions
- Error mapping functions
- Any helper logic used in multiple places

**Code Example - WRONG:**
```rust
// In transformers.rs
fn parse_status(status: &str) -> Result<AttemptStatus> {
    match status {
        "SUCCESS" => Ok(AttemptStatus::Charged),
        "PENDING" => Ok(AttemptStatus::Pending),
        "FAILED" => Ok(AttemptStatus::Failure),
        _ => Ok(AttemptStatus::Pending),
    }
}

// In responses.rs (duplicate implementation!)
fn parse_status(status: &str) -> Result<AttemptStatus> {
    match status {
        "SUCCESS" => Ok(AttemptStatus::Charged),
        "PENDING" => Ok(AttemptStatus::Pending),
        "FAILED" => Ok(AttemptStatus::Failure),
        _ => Ok(AttemptStatus::Pending),
    }
}
```

**Code Example - CORRECT:**
```rust
// In responses.rs (canonical location - where status type is defined)
pub fn parse_status(status: &str) -> Result<AttemptStatus> {
    match status {
        "SUCCESS" => Ok(AttemptStatus::Charged),
        "PENDING" => Ok(AttemptStatus::Pending),
        "FAILED" => Ok(AttemptStatus::Failure),
        _ => Ok(AttemptStatus::Pending),
    }
}

// In transformers.rs (import and use)
use super::responses::parse_status;

// Usage
let status = parse_status(&response.status)?;
```

**Why This Matters:**
Duplicate functions cause:
- Bug Persistence: Fix applied to one copy but not others
- Inconsistent Behavior: Duplicates may diverge over time
- Maintenance Burden: Need to update multiple locations
- Code Bloat: Unnecessary code repetition
- Testing Gaps: May test one copy but not others

**How to Fix:**
1. Search for duplicate function signatures across module files
2. Compare implementations to ensure they're truly identical
3. Choose canonical location:
   - For status parsing: `responses.rs` (with status enum)
   - For data validation: `requests.rs` (with request structs)
   - For general helpers: Consider `utils.rs` or `helpers.rs`
4. Keep one implementation, make it `pub`
5. In other files, import the function
6. Remove duplicate implementations
7. Run tests to ensure behavior unchanged

**Auto-Fix Rule:**
```
IF function with same signature found in multiple files
THEN suggest: "Implement once in canonical location, import elsewhere"
```

**Related Patterns:**
- See: ANTI-004 (Reuse existing constants)
- See: ANTI-006 (Use existing helper functions)
- Reference: DRY principle, code reuse patterns

**Lessons Learned:**
- Mark functions as `pub` if they might be used from other modules
- Consider extracting common functions to a shared utilities module
- Document where canonical implementations live

**Prevention:**
- Before writing a function, check if it exists elsewhere in the connector
- Use descriptive function names that are easy to search for
- Code review should identify duplicate implementations
- Consider creating a utilities module early in development

---

### ANTI-006: Use existing helper functions instead of reimplementing logic

**Metadata:**
```yaml
id: ANTI-006
category: CODE_QUALITY
severity: WARNING
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: Medium
tags: [code-reuse, helpers, common-utils, efficiency]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Reimplementing common logic inline when helper functions already exist in `domain_types`, `common_utils`, or other common crates leads to code duplication and potential bugs. Always check for existing helper functions before implementing utility logic.

**Context / When This Applies:**
This applies when implementing any utility or common logic:
- Amount formatting and conversion
- Date/time parsing and formatting
- String validation and sanitization
- Data structure conversions
- Common transformations

**Code Example - WRONG:**
```rust
// Manually implementing amount formatting
let formatted = if cents < 10 {
    format!("{}.0{}", dollars, cents)
} else {
    format!("{}.{}", dollars, cents)
};

// Manually parsing dates
let parts: Vec<&str> = date_str.split('-').collect();
let year = parts[0].parse::<i32>()?;
let month = parts[1].parse::<u32>()?;
// ... more manual parsing

// Manually validating email
let has_at = email.contains('@');
let has_dot = email.contains('.');
if !has_at || !has_dot {
    return Err(...);
}
```

**Code Example - CORRECT:**
```rust
// Using existing helper functions from common crates
use domain_types::amount_helpers;
use common_utils::date_helpers;
use common_utils::validation;

// Amount formatting
let formatted = amount_helpers::format_amount(amount, currency)?;

// Date parsing
let date = date_helpers::parse_iso_date(date_str)?;

// Email validation
validation::validate_email(&email)?;

// Or for connector-specific conversions
let minor_amount = MinorUnit::from_major(major_amount, currency)?;
```

**Why This Matters:**
Using existing helpers ensures:
- Correctness: Well-tested implementations
- Consistency: Same logic across all connectors
- Maintainability: Bugs fixed in one place benefit everyone
- Efficiency: Optimized implementations
- Code Reduction: Less code to maintain

**How to Fix:**
1. Before implementing any utility logic, check for existing helpers:
   - Search `domain_types` crate for domain-specific helpers
   - Search `common_utils` crate for general utilities
   - Check `common_enums` for standard enumerations
   - Review other connectors for patterns
2. If helper exists:
   - Import it: `use domain_types::helpers::function_name;`
   - Use it instead of custom implementation
   - Remove custom logic
3. If no helper exists but logic is common:
   - Consider proposing a new helper function for common crates
   - Document the need in code review
4. For connector-specific logic, create local helper in `utils.rs`

**Auto-Fix Rule:**
```
IF code implements amount/date/validation logic manually
THEN suggest: "Check domain_types, common_utils for existing helper functions"
```

**Related Patterns:**
- See: ANTI-005 (Remove duplicate functions)
- See: ANTI-004 (Reuse existing constants)
- Reference: domain_types documentation
- Reference: common_utils documentation

**Lessons Learned:**
- Explore common crates before implementing utilities
- Helper functions are often more robust than quick implementations
- Ask in code review if unsure whether a helper exists

**Prevention:**
- During onboarding, review available helper functions in common crates
- Keep a reference list of commonly-needed helpers
- Code review should suggest existing helpers when applicable
- Search codebase before implementing any utility function

---

## Connector Pattern Anti-Patterns

### ANTI-007: Use correct authentication configuration - avoid hardcoding and verify required fields

**Metadata:**
```yaml
id: ANTI-007
category: CONNECTOR_PATTERN
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [authentication, security, configuration, connector-pattern]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Hardcoding authentication credentials or using incorrect auth type structure (e.g., expecting both `api_key` and `secret` when only `api_key` is needed) causes security issues and authentication failures. Authentication configuration must exactly match the connector's requirements.

**Context / When This Applies:**
This applies when implementing connector authentication:
- Extracting auth credentials from `auth_type`
- Building authorization headers
- Configuring authentication method
- Any security credential handling

**Code Example - WRONG:**
```rust
// Hardcoding auth credentials - NEVER DO THIS!
let auth_header = "Bearer hardcoded-key-123";

// Or using wrong structure with unnecessary fields
let auth = ConnectorAuthType::HeaderKey {
    api_key: key,
    secret: secret,  // Not needed for this connector!
};

// Or not extracting from auth_type
impl {{ConnectorName}} {
    fn build_headers() -> Headers {
        Headers {
            authorization: "Bearer test-key".to_string(),  // Wrong!
        }
    }
}
```

**Code Example - CORRECT:**
```rust
// Extract auth from auth_type
let auth = {{ConnectorName}}AuthType::try_from(auth_type)
    .change_context(errors::ConnectorError::InvalidAuthType)?;

// Use only required fields based on connector requirements
let auth_header = match auth {
    {{ConnectorName}}AuthType::HeaderKey { api_key } => {
        format!("Bearer {}", api_key.peek())
    }
};

// Or for more complex auth
let auth = ConnectorAuthType::BodyKey {
    api_key: router_data.connector_auth_type.api_key.clone(),
    key1: router_data.connector_auth_type.key1.clone(),
};
```

**Why This Matters:**
Correct authentication configuration ensures:
- Security: Credentials not exposed in code
- Functionality: Connector accepts authentication
- Configurability: Different credentials per environment
- Maintainability: Credentials managed centrally
- Compliance: Proper credential handling

Wrong authentication causes:
- Authentication failures (401/403 errors)
- Security vulnerabilities (exposed credentials)
- Configuration inflexibility
- Production incidents

**How to Fix:**
1. Review connector API documentation for auth requirements
2. Determine correct `ConnectorAuthType` variant:
   - `HeaderKey { api_key }` - API key in header
   - `BodyKey { api_key, key1 }` - Multiple keys
   - `SignatureKey { ... }` - Signature-based auth
3. Extract auth from `router_data.connector_auth_type`
4. Use `.peek()` to access secret values safely
5. Build headers/body with extracted credentials
6. Remove any hardcoded credentials
7. Test with real credentials in test environment

**Auto-Fix Rule:**
```
IF hardcoded string looks like API key/token/credential
THEN error: "Never hardcode credentials - extract from auth_type"

IF ConnectorAuthType has unused fields
THEN suggest: "Remove unnecessary auth fields, use only what connector requires"
```

**Related Patterns:**
- See: ANTI-001 (Avoid hardcoding values)
- See: SEC-001 (Avoid unsafe code)
- Reference: UCS authentication patterns

**Lessons Learned:**
- Always use `.peek()` to access Secret<> values
- Different connectors need different auth structures
- Test authentication early to catch configuration issues

**Prevention:**
- Review connector API auth documentation during design phase
- Never commit code with hardcoded credentials (use git hooks)
- Use proper auth types from the start
- Test with real auth configuration

---

### ANTI-008: Map unknown/error connector statuses to Pending, not Failure

**Metadata:**
```yaml
id: ANTI-008
category: CONNECTOR_PATTERN
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [status-mapping, state-machine, retry-logic, connector-pattern]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Mapping unknown or unrecognized connector statuses to `Failed`/`Failure` status prevents retry logic and causes premature payment failures. Unknown or ambiguous statuses should be mapped to `Pending` to allow the system to retry status checks and properly resolve the final status.

**Context / When This Applies:**
This applies when mapping connector statuses to UCS `AttemptStatus`:
- Unknown status codes from connector
- Error responses with ambiguous meaning
- Timeout or communication errors
- Any status where the final outcome is uncertain

**Code Example - WRONG:**
```rust
// Mapping unknowns to Failure - prevents retry!
impl From<{{ConnectorName}}Status> for AttemptStatus {
    fn from(status: {{ConnectorName}}Status) -> Self {
        match status {
            {{ConnectorName}}Status::Success => AttemptStatus::Charged,
            {{ConnectorName}}Status::Failed => AttemptStatus::Failure,
            {{ConnectorName}}Status::Unknown => AttemptStatus::Failure,  // Wrong!
            {{ConnectorName}}Status::Error => AttemptStatus::Failure,    // Wrong!
            {{ConnectorName}}Status::Processing => AttemptStatus::Pending,
        }
    }
}
```

**Code Example - CORRECT:**
```rust
// Map unknowns to Pending - allows retry
impl From<{{ConnectorName}}Status> for AttemptStatus {
    fn from(status: {{ConnectorName}}Status) -> Self {
        match status {
            {{ConnectorName}}Status::Success => AttemptStatus::Charged,
            {{ConnectorName}}Status::Completed => AttemptStatus::Charged,
            {{ConnectorName}}Status::Failed => AttemptStatus::Failure,
            {{ConnectorName}}Status::Declined => AttemptStatus::Failure,
            {{ConnectorName}}Status::Cancelled => AttemptStatus::Voided,
            // Map ambiguous statuses to Pending
            {{ConnectorName}}Status::Unknown => AttemptStatus::Pending,
            {{ConnectorName}}Status::Error => AttemptStatus::Pending,
            {{ConnectorName}}Status::Processing => AttemptStatus::Pending,
            // Default to Pending for unrecognized statuses
            _ => AttemptStatus::Pending,
        }
    }
}
```

**Why This Matters:**
Payment processing is asynchronous and may have transient issues:
- Network timeouts don't mean payment failed
- Connector may be temporarily unavailable
- Status may not be final yet
- Retry logic can resolve ambiguous states

Mapping to Failure prematurely:
- Prevents retry attempts
- Causes customer-facing payment failures
- Loses potentially successful payments
- Breaks payment recovery logic
- Impacts payment success rates

Mapping to Pending allows:
- System to retry status check
- Time for connector to finalize status
- Proper resolution of transient issues
- Higher payment success rates

**How to Fix:**
1. Review all status mapping in `From<ConnectorStatus> for AttemptStatus`
2. Identify terminal success statuses → map to `Charged`/`Authorized`/etc.
3. Identify terminal failure statuses → map to `Failure` (only if connector explicitly indicates failure)
4. Identify all non-terminal/ambiguous statuses → map to `Pending`:
   - Unknown
   - Error (unless explicitly terminal)
   - Processing/InProgress
   - Timeout
   - Any status with uncertain outcome
5. Add default case mapping to `Pending`:
   ```rust
   _ => AttemptStatus::Pending
   ```
6. Document which statuses are terminal vs. retriable

**Auto-Fix Rule:**
```
IF status matches "Unknown|Error|Timeout" AND mapped to Failure
THEN suggest: "Map to Pending to allow retry logic"

IF match has no default case
THEN suggest: "Add default case: _ => AttemptStatus::Pending"
```

**Related Patterns:**
- See: ANTI-009 (Refund status to Charged)
- See: Flow-specific status mapping patterns
- Reference: UCS state machine documentation

**Lessons Learned:**
- When in doubt, map to Pending rather than Failure
- Only map to Failure for explicit terminal failure statuses
- Default case should be Pending, not Failure
- PSync flow will eventually resolve Pending statuses

**Prevention:**
- Review connector API documentation for status meanings
- Test with simulated timeouts and errors
- Verify retry logic works with Pending statuses
- Monitor payment success rates after status mapping changes

---

### ANTI-009: Map refund statuses to Charged state - refunds only occur on charged payments

**Metadata:**
```yaml
id: ANTI-009
category: CONNECTOR_PATTERN
severity: CRITICAL
connector: general
flow: Authorize
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [status-mapping, refunds, state-machine, payment-states]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Mapping refund-related payment statuses (`SentForRefund`, `RefundFailed`, `Refunded`) to non-charged states like `Pending` or `Failure` violates payment state machine logic. Refunds can only occur on payments that have already been charged, so any refund-related status must map to `Charged` state for the payment itself.

**Context / When This Applies:**
This applies when mapping connector payment statuses that indicate refund operations:
- `SentForRefund` / `RefundPending` / `RefundProcessing`
- `RefundFailed` / `RefundRejected`
- `Refunded` / `RefundCompleted`
- `PartiallyRefunded`

Note: These are payment statuses, not refund object statuses. The refund itself has separate status tracking.

**Code Example - WRONG:**
```rust
// Incorrect mapping of refund statuses
impl From<{{ConnectorName}}PaymentStatus> for AttemptStatus {
    fn from(status: {{ConnectorName}}PaymentStatus) -> Self {
        match status {
            Status::Authorized => AttemptStatus::Authorized,
            Status::Charged => AttemptStatus::Charged,
            Status::SentForRefund => AttemptStatus::Pending,     // Wrong!
            Status::RefundFailed => AttemptStatus::Failure,      // Wrong!
            Status::Refunded => AttemptStatus::Success,          // Wrong!
            Status::PartiallyRefunded => AttemptStatus::PartialCharged, // Wrong!
        }
    }
}
```

**Code Example - CORRECT:**
```rust
// Correct mapping - refund statuses indicate payment is Charged
impl From<{{ConnectorName}}PaymentStatus> for AttemptStatus {
    fn from(status: {{ConnectorName}}PaymentStatus) -> Self {
        match status {
            Status::Authorized => AttemptStatus::Authorized,
            Status::Captured => AttemptStatus::Charged,
            Status::Charged => AttemptStatus::Charged,
            // All refund-related statuses map to Charged
            // (the payment itself is charged, refund has separate tracking)
            Status::SentForRefund => AttemptStatus::Charged,
            Status::RefundFailed => AttemptStatus::Charged,
            Status::Refunded => AttemptStatus::Charged,
            Status::PartiallyRefunded => AttemptStatus::Charged,
        }
    }
}
```

**Why This Matters:**
Payment state machine correctness:
- A refund can only be initiated on a captured/charged payment
- If payment shows `Refunded`, it means it WAS charged (and now has refund)
- Payment status and refund status are separate concerns
- Mapping to non-Charged state breaks state machine invariants

Incorrect mapping causes:
- State machine violations
- Incorrect payment lifecycle tracking
- Reporting inconsistencies
- Reconciliation failures
- Confusion about payment vs. refund status

**How to Fix:**
1. Review payment status mapping for refund-related statuses
2. Identify statuses that indicate refund operations:
   - Look for keywords: Refund, Reversed, Chargeback
3. Map ALL refund-related payment statuses to `Charged`:
   ```rust
   Status::Refunded => AttemptStatus::Charged,
   Status::PartiallyRefunded => AttemptStatus::Charged,
   Status::RefundPending => AttemptStatus::Charged,
   Status::RefundFailed => AttemptStatus::Charged,
   ```
4. Ensure refund objects have separate status tracking (RSync flow)
5. Document that payment status reflects payment state, not refund state

**Auto-Fix Rule:**
```
IF payment status contains "Refund" AND NOT mapped to Charged
THEN suggest: "Map refund payment statuses to Charged - refunds only occur on charged payments"
```

**Related Patterns:**
- See: ANTI-008 (Map unknown to Pending)
- See: RSync flow patterns for refund status tracking
- Reference: UCS payment state machine documentation

**Lessons Learned:**
- Payment status and refund status are orthogonal
- A "Refunded" payment status means: "This payment was charged and has refunds"
- The refund itself has status tracked separately in refund objects
- State machine transitions must be valid: can't refund an uncharged payment

**Prevention:**
- Review payment state machine during design phase
- Clearly separate payment status mapping from refund status mapping
- Test refund flows to verify status transitions
- Document payment vs. refund status distinction

---

## Rust Best Practice Anti-Patterns

### ANTI-010: Replace single-variant enums with constant strings

**Metadata:**
```yaml
id: ANTI-010
category: RUST_BEST_PRACTICE
severity: WARNING
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: Low
tags: [rust-idioms, enums, constants, code-simplification]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Using an enum with only one variant adds unnecessary complexity and allocation overhead. If there's only one possible value, use a constant string instead. Enums should only be used when there are multiple variants representing different states or types.

**Context / When This Applies:**
This applies when defining types with only one possible value:
- Payment type (if only "card" is supported)
- Transaction type (if only "sale" is supported)
- Any field with a single fixed value

**Code Example - WRONG:**
```rust
// Single-variant enum - unnecessary complexity
#[derive(Serialize, Deserialize)]
enum PaymentType {
    Card,
}

#[derive(Serialize)]
struct Request {
    payment_type: PaymentType,
}

let request = Request {
    payment_type: PaymentType::Card,
};
```

**Code Example - CORRECT:**
```rust
// Use a constant instead
const PAYMENT_TYPE: &str = "card";

#[derive(Serialize)]
struct Request {
    #[serde(rename = "paymentType")]
    payment_type: &'static str,
}

let request = Request {
    payment_type: PAYMENT_TYPE,
};

// Or inline if used once
#[derive(Serialize)]
struct Request {
    #[serde(rename = "paymentType")]
    payment_type: &'static str,
}
```

**Why This Matters:**
- Simplicity: Constants are simpler than enums
- Performance: No enum allocation/matching overhead
- Code Size: Less code to maintain
- Clarity: Intent is clearer with a constant

Single-variant enums are only justified if:
- You expect more variants in the future
- You need trait implementations specific to the type
- The type system benefit is significant

**How to Fix:**
1. Find enums with only one variant
2. Replace with `const` declaration:
   ```rust
   const TYPE_NAME: &str = "value";
   ```
3. Update struct fields to use `&'static str` or `String`
4. Replace enum usage with constant
5. Remove enum definition
6. Update serialization if needed

**Auto-Fix Rule:**
```
IF enum has exactly one variant
THEN suggest: "Replace with const string unless multiple variants expected"
```

**Related Patterns:**
- See: ANTI-004 (Reuse existing constants)
- Reference: Rust API guidelines on enums

**Lessons Learned:**
- Don't create enums "for future expansion" - add variants when needed
- Constants are often more appropriate for single values
- Type safety doesn't always require enums

**Prevention:**
- Before creating an enum, consider if multiple variants exist
- Use constants for single-value types
- Add enums when second variant is actually needed

---

### ANTI-011: Use ? operator for error propagation instead of explicit return

**Metadata:**
```yaml
id: ANTI-011
category: RUST_BEST_PRACTICE
severity: WARNING
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: Medium
tags: [rust-idioms, error-handling, error-stack, code-clarity]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Using explicit `return Err(...)` statements for error propagation is less idiomatic and provides worse error context than using the `?` operator. The `?` operator provides better error stack traces through `error_stack`, which is crucial for debugging connector issues.

**Context / When This Applies:**
This applies to all error propagation in connector code:
- Validation errors
- Missing field errors
- Conversion errors
- Any error that should bubble up to the caller

**Code Example - WRONG:**
```rust
// Explicit return - less idiomatic, worse stack traces
fn validate_request(data: &RouterDataV2<...>) -> Result<()> {
    if some_condition {
        return Err(errors::ConnectorError::InvalidDataFormat {
            field_name: "collection_reference not allowed",
        }.into());
    }

    if other_condition {
        return Err(errors::ConnectorError::MissingRequiredField {
            field_name: "transaction_id",
        }.into());
    }

    Ok(())
}
```

**Code Example - CORRECT:**
```rust
// Using ? operator - idiomatic, better error context
fn validate_request(data: &RouterDataV2<...>) -> Result<()> {
    if some_condition {
        Err(errors::ConnectorError::InvalidDataFormat {
            field_name: "collection_reference not allowed",
        })?;
    }

    // Or even better with early return style
    (!other_condition)
        .then_some(())
        .ok_or(errors::ConnectorError::MissingRequiredField {
            field_name: "transaction_id",
        })?;

    Ok(())
}

// Best pattern for Result chaining
fn convert_amount(amount: i64) -> Result<MinorUnit> {
    MinorUnit::try_from(amount)
        .change_context(errors::ConnectorError::InvalidDataFormat {
            field_name: "amount",
        })
}
```

**Why This Matters:**
The `?` operator provides:
- Better Error Context: `error_stack` adds location information automatically
- Idiomatic Rust: Standard pattern for error propagation
- Cleaner Code: Less verbose than explicit returns
- Better Stack Traces: Full error chain preserved
- Debugging: Easier to trace error origins

**How to Fix:**
1. Find explicit `return Err(...)` statements
2. Replace with `Err(...)?` pattern:
   ```rust
   // Before
   return Err(error.into());

   // After
   Err(error)?;
   ```
3. For Result chains, use `.change_context()` instead of `.map_err()`:
   ```rust
   // Before
   value.ok_or(error).map_err(|e| e.into())?

   // After
   value.ok_or(error)?
   ```
4. Consider using helper methods for common error patterns

**Auto-Fix Rule:**
```
IF code contains "return Err(" for error propagation
THEN suggest: "Use ? operator: Err(...)?;"
```

**Related Patterns:**
- See: Rust error handling best practices
- Reference: error_stack documentation
- Reference: UCS error handling patterns

**Lessons Learned:**
- The `?` operator is not just syntax sugar - it adds error context
- `error_stack` integration requires using `?` for full benefit
- Explicit returns are sometimes needed, but rarely for simple error propagation

**Prevention:**
- Use `?` operator by default for error propagation
- Only use explicit return when you need to do something before returning
- Code review should catch explicit error returns
- Learn `error_stack` patterns for better error handling

---

---

---

# 6. SUCCESS PATTERNS

> **Purpose:** Celebrate and document excellent implementations for others to learn from

**Status:** Ready for population - Add success patterns from exemplary implementations

**Organization:**
- Excellent Transformer Designs
- Exceptional Error Handling
- Reusable Code Patterns
- Comprehensive Test Coverage
- Well-Documented Complex Logic

**Guidance:**
- Document what was done exceptionally well
- Explain why it's excellent
- Note reusability potential
- Provide context for learning

**[Content will be added here based on implementation learnings]**

---

---

# 7. HISTORICAL FEEDBACK ARCHIVE

> **Purpose:** Archive of resolved issues and deprecated patterns for historical reference

**Status:** Ready for population - Archive resolved patterns and outdated guidance

**Organization:**
- Resolved Issues (Fixed and no longer applicable)
- Deprecated Patterns (Old patterns replaced by better ones)
- Historical Context (Why certain decisions were made)

**Guidance:**
- Move resolved patterns here with resolution date
- Document why patterns became deprecated
- Preserve historical context for learning
- Note migration paths from old to new patterns

**[Content will be added here based on implementation history]**

---

---

# 8. SECURITY GUIDELINES

> **Purpose:** Critical security patterns and anti-patterns for connector implementations

**Status:** Active - Contains 2 critical security guidelines

---

### SEC-001: Avoid using unsafe code blocks in connector implementations

**Metadata:**
```yaml
id: SEC-001
category: SECURITY
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [security, unsafe-code, memory-safety, critical]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Using `unsafe` blocks for memory operations, pointer dereferencing, or type casting in connector implementations introduces potential memory safety issues and security vulnerabilities. Rust's type system provides safety guarantees that unsafe code undermines.

**Context / When This Applies:**
This applies to ANY use of unsafe code in connector implementations:
- `unsafe { }` blocks
- Pointer dereferencing
- Memory transmutation
- Raw pointer operations
- Manual memory management

**Code Example - WRONG:**
```rust
// Using unsafe for memory operations - NEVER DO THIS!
unsafe {
    std::mem::transmute::<_, SomeType>(data)
}

// Or unsafe pointer operations
unsafe {
    std::ptr::read(ptr)
}

// Or unsafe type casting
unsafe {
    *(value as *const A as *const B)
}
```

**Code Example - CORRECT:**
```rust
// Use safe Rust alternatives
use std::convert::TryFrom;

// Safe conversion with error handling
let result = SomeType::try_from(data)?;

// Safe string conversion
let string = String::from_utf8_lossy(&bytes);

// Safe collection conversion
let boxed = Vec::into_boxed_slice();

// Derive traits for safe operations
#[derive(Clone, Copy)]
struct MyStruct {
    field: i64,
}
```

**Why This Matters:**
Unsafe code undermines Rust's safety guarantees:
- Memory Safety: Can cause undefined behavior, segfaults, data corruption
- Security: Can introduce exploitable vulnerabilities
- Maintainability: Harder to reason about correctness
- Debugging: Undefined behavior is extremely hard to debug
- Compliance: May violate security requirements

In payment processing:
- Security is paramount - handling sensitive financial data
- Undefined behavior could corrupt payment data
- Memory safety issues can lead to data leaks
- Vulnerabilities can be exploited by attackers

**How to Fix:**
1. Find all `unsafe` blocks in connector code
2. For each unsafe block, identify what it's trying to do
3. Replace with safe alternatives:
   - Type conversion → `TryFrom`, `From`, or explicit conversion
   - String handling → `String::from_utf8_lossy`, `str::from_utf8`
   - Memory allocation → Use `Vec`, `Box`, or other smart pointers
   - Trait requirements → Derive `Clone`, `Copy`, or implement safely
4. If you think unsafe is truly necessary:
   - Document WHY it's needed
   - Get security review approval
   - Add extensive comments explaining safety invariants
   - Consider if there's a safe alternative you haven't found
5. Remove unsafe blocks and test thoroughly

**Auto-Fix Rule:**
```
IF code contains "unsafe {" in connector implementation
THEN error: "CRITICAL - Unsafe code not allowed in connectors. Use safe Rust alternatives."
```

**Related Patterns:**
- See: SEC-002 (Avoid direct memory manipulation)
- See: ANTI-007 (Correct authentication - security concern)
- Reference: Rust unsafe code guidelines
- Reference: UCS security requirements

**Lessons Learned:**
- There are safe alternatives for nearly all operations in connector code
- Unsafe code is almost never justified in connector implementations
- Rust's type system is designed to make safe code possible
- Payment processing code should never use unsafe operations

**Prevention:**
- Code review must reject any unsafe code without security approval
- Add linter rules to flag unsafe blocks
- Never copy unsafe code from examples without understanding it
- If you think you need unsafe, ask for help finding safe alternatives
- Security review required for any unsafe code

---

### SEC-002: Avoid direct memory manipulation in connector code

**Metadata:**
```yaml
id: SEC-002
category: SECURITY
severity: CRITICAL
connector: general
flow: All
applicability: ALL_CONNECTORS
date_added: 2025-10-14
status: Active
frequency: 1
impact: High
tags: [security, memory-manipulation, undefined-behavior, critical]
source_pr: juspay/connector-service#216
source_connector: worldpay
reviewer: jarnura
```

**Issue Description:**
Direct manipulation of memory using pointer arithmetic, `transmute`, or manual memory management bypasses Rust's safety guarantees and can lead to undefined behavior, data corruption, or security vulnerabilities. Always use high-level abstractions provided by the standard library.

**Context / When This Applies:**
This applies to any low-level memory operations:
- `std::mem::transmute`
- Pointer arithmetic
- Manual memory allocation/deallocation
- Raw pointer manipulation
- Direct memory access
- Uninitialized memory usage

**Code Example - WRONG:**
```rust
// Direct memory manipulation - NEVER DO THIS!
use std::mem;

// Transmute - extremely dangerous
let value: B = mem::transmute::<A, B>(original);

// Raw pointer manipulation
unsafe {
    *ptr = new_value;
}

// Manual memory management
unsafe {
    let layout = std::alloc::Layout::new::<MyStruct>();
    let ptr = std::alloc::alloc(layout);
    // ... manual memory management
}

// Uninitialized memory
let mut uninit: MaybeUninit<MyStruct> = MaybeUninit::uninit();
unsafe { uninit.assume_init() }
```

**Code Example - CORRECT:**
```rust
// Use high-level safe abstractions

// Type conversion - use proper traits
impl From<A> for B {
    fn from(a: A) -> B {
        B {
            field1: a.field1,
            field2: a.field2.to_string(),
        }
    }
}
let converted: B = B::from(original);

// String handling - safe methods
let string = String::from("value");
let bytes = string.as_bytes();
let from_bytes = String::from_utf8_lossy(bytes);

// Collections - use standard library
let vec = vec![1, 2, 3];
let boxed = vec.into_boxed_slice();

// Copying - derive Clone/Copy traits
#[derive(Clone, Copy)]
struct MyStruct {
    field: i64,
}

// Initialization - use default or constructors
let value = MyStruct::default();
// or
let value = MyStruct::new(args);
```

**Why This Matters:**
Direct memory manipulation causes severe issues:
- Undefined Behavior: Can cause any kind of corruption or crash
- Security Vulnerabilities: Exploitable by attackers
- Data Corruption: Payment data could be corrupted
- Memory Leaks: Manual management often leaks memory
- Race Conditions: Improper synchronization in concurrent code
- Debugging Nightmare: Extremely hard to trace and fix

In payment processing:
- Data integrity is critical - corruption could lose money
- Security vulnerabilities could expose customer data
- Undefined behavior could affect multiple transactions
- Memory safety is a compliance requirement

**How to Fix:**
1. Find all uses of:
   - `std::mem::transmute`
   - `std::ptr::read`, `std::ptr::write`
   - `std::alloc` functions
   - Raw pointer operations
   - `MaybeUninit::assume_init`
2. Replace with safe alternatives:
   - Type conversion → Implement `From`/`TryFrom` traits
   - Memory allocation → Use `Vec`, `Box`, `String`, etc.
   - Data copying → Derive `Clone`/`Copy` or use `.clone()`
   - Initialization → Use `Default::default()` or constructors
3. Refactor logic to use high-level abstractions
4. Remove all direct memory manipulation
5. Test thoroughly to ensure correct behavior

**Auto-Fix Rule:**
```
IF code contains ("transmute"|"ptr::read"|"ptr::write"|"alloc::alloc")
THEN error: "CRITICAL - Direct memory manipulation not allowed. Use safe standard library types."
```

**Related Patterns:**
- See: SEC-001 (Avoid unsafe code)
- See: ANTI-007 (Correct authentication configuration)
- Reference: Rust ownership and borrowing
- Reference: Rust standard library documentation

**Lessons Learned:**
- Rust standard library provides safe abstractions for all common operations
- Manual memory management is unnecessary in connector code
- High-level types (`Vec`, `String`, `Box`) are safe and efficient
- `transmute` is almost never the right solution
- If you're manually managing memory, you're doing it wrong

**Prevention:**
- Never use low-level memory operations in connector code
- Use standard library types exclusively
- Implement proper traits (`From`, `TryFrom`, `Clone`) for conversions
- Code review must reject direct memory manipulation
- Security review required if you think low-level operations are needed
- Learn Rust ownership model to work with the type system, not against it

---

---

# 📈 APPENDIX: METRICS & TRACKING

## Feedback Statistics

**Total Feedback Entries:** 17

**By Category:**
- UCS_PATTERN_VIOLATION: 2 (UCS-001, UCS-002)
- CONNECTOR_PATTERN: 5 (UCS-003, UCS-004, ANTI-007, ANTI-008, ANTI-009)
- CODE_QUALITY: 6 (ANTI-001, ANTI-002, ANTI-003, ANTI-004, ANTI-005, ANTI-006)
- RUST_BEST_PRACTICE: 2 (ANTI-010, ANTI-011)
- SECURITY: 2 (SEC-001, SEC-002)
- TESTING_GAP: 0
- DOCUMENTATION: 0
- PERFORMANCE: 0
- SUCCESS_PATTERN: 0

**By Severity:**
- CRITICAL: 11 (UCS-001, UCS-002, UCS-003, ANTI-001, ANTI-002, ANTI-003, ANTI-007, ANTI-008, ANTI-009, SEC-001, SEC-002)
- WARNING: 6 (UCS-004, ANTI-004, ANTI-005, ANTI-006, ANTI-010, ANTI-011)
- SUGGESTION: 0
- INFO: 0

**By Section:**
- Section 1 (Critical Patterns): 1 (FB-001 example provided)
- Section 2 (UCS-Specific Guidelines): 4 (UCS-001 to UCS-004)
- Section 3 (Flow-Specific Best Practices): 0 (awaiting population)
- Section 4 (Payment Method Patterns): 0 (awaiting population)
- Section 5 (Common Anti-Patterns): 11 (ANTI-001 to ANTI-011)
- Section 6 (Success Patterns): 0 (awaiting population)
- Section 7 (Historical Archive): 0 (awaiting population)
- Section 8 (Security Guidelines): 2 (SEC-001 to SEC-002)

**Most Frequent Issues:**
All entries have frequency: 1 (first occurrence from worldpay connector review)

**Source Information:**
- Source PR: juspay/connector-service#216
- Source Connector: worldpay
- Reviewer: jarnura
- Date Added: 2025-10-14

**Coverage:**
- Total IDs assigned: 17
- ID ranges used:
  - UCS-001 to UCS-004 (UCS-Specific Guidelines)
  - ANTI-001 to ANTI-011 (Common Anti-Patterns)
  - SEC-001 to SEC-002 (Security Guidelines)
- FB-ID ranges available for future use:
  - FB-001 to FB-099 (Critical UCS Pattern Violations)
  - FB-104 to FB-199 (More UCS-Specific Guidelines)
  - FB-200 to FB-299 (Flow-Specific Best Practices)
  - FB-300 to FB-399 (Payment Method Patterns)
  - FB-411 to FB-499 (More Common Anti-Patterns)
  - FB-500 to FB-599 (Success Patterns)
  - FB-600 to FB-699 (Rust Best Practices)
  - FB-700 to FB-799 (Performance Patterns)
  - FB-802 to FB-899 (More Security Guidelines)
  - FB-900 to FB-999 (Testing Patterns)

---

## Version History

**v1.1.0** - 2025-10-14
- Added 17 feedback entries from worldpay connector review
- Populated Section 2: UCS-Specific Guidelines (UCS-001 to UCS-004)
- Populated Section 5: Common Anti-Patterns (ANTI-001 to ANTI-011)
- Added Section 8: Security Guidelines (SEC-001 to SEC-002)
- Updated statistics and metrics
- Source: juspay/connector-service#216 (worldpay), reviewer: jarnura

**v1.0.0** - 2024-MM-DD
- Initial structure created
- Quality review template defined
- Category taxonomy established
- Ready for population with real feedback

---

**End of Feedback Database**
