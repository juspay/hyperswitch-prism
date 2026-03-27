# Contributing to the Feedback Database

This guide explains how to add, update, and manage feedback entries in the UCS Connector Code Quality Feedback Database.

---

## 📋 Table of Contents

1. [When to Add Feedback](#when-to-add-feedback)
2. [Feedback Entry Structure](#feedback-entry-structure)
3. [Step-by-Step Guide](#step-by-step-guide)
4. [Feedback ID System](#feedback-id-system)
5. [Category Guidelines](#category-guidelines)
6. [Severity Assignment](#severity-assignment)
7. [Writing Effective Feedback](#writing-effective-feedback)
8. [Examples](#examples)
9. [Maintenance](#maintenance)

---

## When to Add Feedback

Add feedback entries in these scenarios:

### ✅ Do Add Feedback For:

- **Recurring Issues**: Same problem appears in multiple connectors
- **UCS Pattern Violations**: Deviations from UCS architecture
- **Common Mistakes**: Errors that developers frequently make
- **Best Practices**: Patterns that work exceptionally well
- **Anti-Patterns**: Approaches that should be avoided
- **Security Concerns**: Security-related issues or best practices
- **Performance Issues**: Performance anti-patterns or optimizations
- **New Insights**: Lessons learned during implementation

### ❌ Don't Add Feedback For:

- **One-off Issues**: Problems specific to a single connector
- **Unclear Patterns**: Not yet validated or understood
- **Temporary Workarounds**: Solutions that will change
- **Connector-Specific Logic**: Business logic unique to one connector
- **External Dependencies**: Issues outside our codebase

---

## Feedback Entry Structure

Every feedback entry must follow this structure:

**Template:**

```markdown
### FB-[ID]: [Brief Descriptive Title]

**Metadata:**
```yaml
id: FB-XXX
category: [CATEGORY]
severity: CRITICAL | WARNING | SUGGESTION | INFO
connector: [name] | general
flow: [FlowName] | All
date_added: YYYY-MM-DD
status: Active | Resolved | Archived
frequency: [number]
impact: High | Medium | Low
tags: [tag1, tag2, tag3]
```

**Issue Description:**
[1-2 sentence clear description]

**Context / When This Applies:**
[When does this issue occur or when should this pattern be used]

**Code Example - WRONG (if applicable):**
```rust
// Incorrect implementation
```

**Code Example - CORRECT:**
```rust
// Correct implementation
```

**Why This Matters:**
[Impact and consequences]

**How to Fix:**
1. [Step 1]
2. [Step 2]
3. [Step 3]

**Auto-Fix Rule (if applicable):**
```
IF [condition]
THEN [action]
```

**Related Patterns:**
- See: [reference 1]
- See: [reference 2]

**Lessons Learned:**
[Key takeaways]

**Prevention:**
[How to avoid this in future]

---
```

---

## Step-by-Step Guide

### Step 1: Identify the Pattern

1. **Observe the issue or pattern** during implementation or review
2. **Validate it's recurring** or significant enough to document
3. **Understand the root cause** and correct solution

### Step 2: Choose Feedback ID

Follow the [Feedback ID System](#feedback-id-system):

```
UCS-XXX:     UCS-Specific Architectural Guidelines
ANTI-XXX:    Common Anti-Patterns to Avoid
SEC-XXX:     Security Guidelines and Patterns
FLOW-XXX:    Flow-Specific Best Practices
METHOD-XXX:  Payment Method Patterns
SUCCESS-XXX: Success Patterns and Examples
PERF-XXX:    Performance Patterns and Optimizations
TEST-XXX:    Testing Patterns and Gaps
DOC-XXX:     Documentation Patterns
```

**Example:** If adding a UCS architectural guideline, use next available ID in UCS-XXX range (e.g., UCS-005 if UCS-001 to UCS-004 exist).

### Step 3: Categorize and Set Severity

**Choose Category:** (See [Category Guidelines](#category-guidelines))
- UCS_PATTERN_VIOLATION
- RUST_BEST_PRACTICE
- CONNECTOR_PATTERN
- CODE_QUALITY
- TESTING_GAP
- DOCUMENTATION
- PERFORMANCE
- SECURITY
- SUCCESS_PATTERN

**Assign Severity:** (See [Severity Assignment](#severity-assignment))
- CRITICAL: Blocks UCS compliance, breaks architecture
- WARNING: Suboptimal but functional
- SUGGESTION: Nice-to-have improvement
- INFO: Positive feedback for success patterns

### Step 4: Fill Metadata

```yaml
id: ANTI-012                         # Next available ID in appropriate category
category: CONNECTOR_PATTERN          # Primary category
severity: WARNING                    # Based on impact
connector: general                   # 'general' or specific connector name
flow: Authorize                      # Specific flow or 'All'
date_added: 2024-01-15              # Today's date
status: Active                       # Usually 'Active' for new entries
frequency: 1                         # Start at 1, increment when observed again
impact: Medium                       # High | Medium | Low
tags: [status-mapping, transformers] # Relevant searchable tags
```

### Step 5: Write Clear Description

**Good Description:**
> "Status mapping uses hardcoded string comparisons instead of enum matching, making it error-prone and difficult to maintain."

**Bad Description:**
> "The status thing is wrong."

**Tips:**
- Be specific about what's wrong or what's good
- State the problem or pattern clearly
- Avoid vague language

### Step 6: Provide Code Examples

**Always include:**
- WRONG code (if documenting an issue)
- CORRECT code (the right way to do it)
- Relevant context (enough to understand)

**Example:**

```rust
// WRONG - Hardcoded string matching
let status = match response.status.as_str() {
    "success" => AttemptStatus::Charged,
    "fail" => AttemptStatus::Failure,
    _ => AttemptStatus::Pending,
};

// CORRECT - Enum matching with comprehensive coverage
let status = match response.status {
    ConnectorStatus::Success | ConnectorStatus::Completed => AttemptStatus::Charged,
    ConnectorStatus::Pending | ConnectorStatus::Processing => AttemptStatus::Pending,
    ConnectorStatus::Failed | ConnectorStatus::Declined => AttemptStatus::Failure,
    ConnectorStatus::Cancelled => AttemptStatus::Voided,
    ConnectorStatus::RequiresAction => AttemptStatus::AuthenticationPending,
};
```

### Step 7: Explain Why It Matters

**Don't just say it's wrong, explain:**
- What breaks or degrades
- What risks it introduces
- What becomes harder
- What benefits are lost

**Example:**
> "This matters because hardcoded strings are fragile - typos won't be caught at compile time, new statuses require code changes across multiple places, and maintainability suffers as the connector evolves."

### Step 8: Provide Fix Instructions

**Be specific and actionable:**

**Example Format:**

```markdown
**How to Fix:**
1. Define ConnectorStatus enum in transformers.rs with all statuses
2. Update response struct to use ConnectorStatus instead of String
3. Implement TryFrom<String> for ConnectorStatus for deserialization
4. Update status mapping to use enum matching
5. Add test cases for all status scenarios
```

### Step 9: Add Auto-Fix Rule (if applicable)

If the issue can be detected and fixed programmatically:

```
Auto-Fix Rule:
IF file contains "match response.status.as_str()"
AND file contains "String" type for status field
THEN suggest: "Replace String status with enum and use enum matching"
CONFIDENCE: Medium
```

### Step 10: Link Related Resources

**Example Format:**

```markdown
**Related Patterns:**
- See: guides/patterns/pattern_authorize.md#status-mapping
- See: FB-025 (similar issue for refund status)
- Reference: https://docs.rs/serde/latest/serde/
```

### Step 11: Add to Appropriate Section

Place your feedback entry in the correct section of `feedback.md`:

1. **Critical Patterns** (Section 1) - For CRITICAL UCS violations
2. **UCS-Specific Guidelines** (Section 2) - For UCS patterns
3. **Flow-Specific Best Practices** (Section 3) - For flow patterns
4. **Payment Method Patterns** (Section 4) - For payment method handling
5. **Common Anti-Patterns** (Section 5) - For anti-patterns
6. **Success Patterns** (Section 6) - For exemplary code
7. **Historical Feedback Archive** (Section 7) - For resolved/deprecated

---

## Feedback ID System

### Semantic Category-Based ID Prefixes

The feedback database uses semantic category-based prefixes instead of numerical ranges. This allows unlimited entries per category and makes IDs more meaningful.

| Prefix | Purpose | Severity Typical | Example |
|--------|---------|------------------|---------|
| UCS-XXX | UCS-Specific Architectural Guidelines | WARNING/CRITICAL | UCS-001: Use amount conversion framework |
| ANTI-XXX | Common Anti-Patterns to Avoid | WARNING/CRITICAL | ANTI-001: Avoid hardcoding values |
| SEC-XXX | Security Guidelines and Patterns | CRITICAL/WARNING | SEC-001: Avoid unsafe code |
| FLOW-XXX | Flow-Specific Best Practices | WARNING/SUGGESTION | FLOW-001: Authorize error handling |
| METHOD-XXX | Payment Method Patterns | WARNING/SUGGESTION | METHOD-001: Card validation |
| SUCCESS-XXX | Success Patterns and Examples | INFO | SUCCESS-001: Excellent transformer |
| PERF-XXX | Performance Patterns and Optimizations | WARNING/SUGGESTION | PERF-001: Avoid allocations in hot path |
| TEST-XXX | Testing Patterns and Gaps | SUGGESTION | TEST-001: Comprehensive coverage |
| DOC-XXX | Documentation Patterns | SUGGESTION | DOC-001: Document complex logic |

### How to Choose an ID

1. Determine which category prefix fits your feedback type
2. Check existing IDs with that prefix (in feedback.md)
3. Choose next available number in sequence
4. Reserve the ID by adding it immediately

**Example Process:**

```bash
# Adding a new UCS architectural guideline
1. Check Section 2 (UCS-Specific Guidelines) of feedback.md
2. Find highest UCS ID (e.g., UCS-004)
3. Use next available (UCS-005)
4. Add your entry with UCS-005

# Adding a new anti-pattern
1. Check Section 5 (Common Anti-Patterns) of feedback.md
2. Find highest ANTI ID (e.g., ANTI-011)
3. Use next available (ANTI-012)
4. Add your entry with ANTI-012
```

### Legacy FB-XXX System

The old FB-XXX numbering system (FB-001 to FB-999) has been replaced by semantic prefixes. All new feedback entries should use category-based prefixes (UCS-XXX, ANTI-XXX, etc.). The FB-XXX range is maintained only for the example in Section 1 of feedback.md.

---

## Category Guidelines

### UCS_PATTERN_VIOLATION

**Use when:**
- Code violates UCS architecture requirements
- Wrong types used (RouterData vs RouterDataV2)
- Wrong imports (hyperswitch_* vs domain_types)
- Missing UCS-specific implementations

**Severity:** Usually CRITICAL or WARNING

**Example:**
- Using ConnectorIntegration instead of ConnectorIntegrationV2

---

### RUST_BEST_PRACTICE

**Use when:**
- Non-idiomatic Rust code
- Performance issues from Rust patterns
- Error handling anti-patterns
- Unsafe code usage

**Severity:** Usually WARNING or SUGGESTION

**Example:**
- Using unwrap() where Result should propagate

---

### CONNECTOR_PATTERN

**Use when:**
- Payment connector implementation patterns
- Transformer design issues
- Authentication handling
- Status mapping problems

**Severity:** WARNING to CRITICAL depending on impact

**Example:**
- Inconsistent error response structure

---

### CODE_QUALITY

**Use when:**
- Code duplication
- Naming issues
- Modularity problems
- Readability concerns

**Severity:** Usually WARNING or SUGGESTION

**Example:**
- Duplicated transformer logic across flows

---

### TESTING_GAP

**Use when:**
- Missing tests
- Insufficient coverage
- Untested edge cases
- Missing integration tests

**Severity:** Usually WARNING or SUGGESTION

**Example:**
- No tests for error scenarios

---

### DOCUMENTATION

**Use when:**
- Missing documentation
- Unclear comments
- Undocumented complexity
- API documentation gaps

**Severity:** Usually SUGGESTION

**Example:**
- Complex transformer without explanation

---

### PERFORMANCE

**Use when:**
- Performance anti-patterns
- Inefficient algorithms
- Unnecessary allocations
- Optimization opportunities

**Severity:** Usually WARNING or SUGGESTION

**Example:**
- Repeated string allocations in loop

---

### SECURITY

**Use when:**
- Security vulnerabilities
- Exposed sensitive data
- Missing validation
- Authentication issues

**Severity:** Usually CRITICAL or WARNING

**Example:**
- API keys logged in error messages

---

### SUCCESS_PATTERN

**Use when:**
- Exemplary implementations
- Reusable patterns
- Excellent practices
- Learning examples

**Severity:** Always INFO (positive feedback)

**Example:**
- Beautifully designed transformer with excellent error handling

---

## Severity Assignment

### 🚨 CRITICAL

**Assign when:**
- Breaks UCS architecture requirements
- Security vulnerabilities exist
- Will cause runtime failures
- Violates core compliance
- Makes code unmaintainable

**Impact:** -20 points per issue

**Examples:**
- Using RouterData instead of RouterDataV2
- Exposed credentials in code
- Missing mandatory trait implementations
- Hardcoded secrets

**Template Phrase:**
> "This is CRITICAL because it [breaks UCS architecture | creates security risk | will fail at runtime | violates core requirements]"

---

### ⚠️ WARNING

**Assign when:**
- Suboptimal but functional
- Creates technical debt
- Maintenance concern
- Performance issue
- Pattern inconsistency

**Impact:** -5 points per issue

**Examples:**
- Code duplication
- Non-idiomatic Rust
- Missing test coverage
- Inefficient transformations

**Template Phrase:**
> "This is a WARNING because it [creates technical debt | harms maintainability | impacts performance | violates best practices]"

---

### 💡 SUGGESTION

**Assign when:**
- Enhancement opportunity
- Code quality improvement
- Documentation addition
- Refactoring opportunity
- Minor optimization

**Impact:** -1 point per issue

**Examples:**
- Better variable names
- Additional comments
- Extracted helper function
- More comprehensive tests

**Template Phrase:**
> "This is a SUGGESTION because it [would improve clarity | enhance maintainability | provide better documentation | optimize slightly]"

---

### ✨ INFO

**Assign when:**
- Exemplary implementation
- Success pattern
- Best practice example
- Reusable pattern
- Learning example

**Impact:** 0 (positive reinforcement)

**Examples:**
- Excellent error handling
- Clean transformer design
- Comprehensive test coverage
- Well-documented complexity

**Template Phrase:**
> "This is EXCELLENT because it [demonstrates best practices | provides reusable pattern | shows exceptional quality | serves as learning example]"

---

## Writing Effective Feedback

### Do's ✅

**Be Specific:**

**Example:**
```markdown
Good: "The status mapping in transformers.rs:45 uses hardcoded string matching which is error-prone"
Bad: "Status mapping is bad"
```

**Provide Context:**

**Example:**
```markdown
Good: "When implementing authorize flow, ensure status field uses enum instead of String to prevent typos"
Bad: "Use enum"
```

**Show Examples:**

**Example:**
```markdown
Good: [Includes both WRONG and CORRECT code examples]
Bad: [Only says "fix the status mapping"]
```

**Explain Impact:**

**Example:**
```markdown
Good: "This matters because typos in status strings won't be caught at compile time, leading to runtime bugs"
Bad: "This is wrong"
```

**Give Action Steps:**

**Example:**
```markdown
Good:
1. Define ConnectorStatus enum
2. Update response struct
3. Implement TryFrom<String>
4. Update matching logic
Bad: "Fix it"
```

### Don'ts ❌

**Don't Be Vague:**

**Example:**
```markdown
Bad: "The code is not good"
Good: "The transformer duplicates currency conversion logic from common_utils"
```

**Don't Assume Knowledge:**

**Example:**
```markdown
Bad: "Just use the standard pattern"
Good: "Use RouterDataV2 instead of RouterData, as required by UCS architecture (see guides/patterns/)"
```

**Don't Skip Examples:**

**Example:**
```markdown
Bad: "Wrong status mapping approach"
Good: [Includes code examples of wrong and correct approaches]
```

**Don't Forget References:**

**Example:**
```markdown
Bad: [No references provided]
Good: "See guides/patterns/pattern_authorize.md#status-mapping for detailed pattern"
```

**Don't Be Prescriptive Without Explanation:**

**Example:**
```markdown
Bad: "Must use enum"
Good: "Use enum instead of String for type safety and compile-time validation"
```

---

## Examples

### Example 1: Critical UCS Pattern Violation

**Complete Example:**

```markdown
### FB-002: Use ConnectorIntegrationV2, Not ConnectorIntegration

**Metadata:**
```yaml
id: FB-002
category: UCS_PATTERN_VIOLATION
severity: CRITICAL
connector: general
flow: All
date_added: 2024-01-15
status: Active
frequency: 1
impact: High
tags: [ucs-architecture, trait-implementation, breaking-change]
```

**Issue Description:**
Connector implementations must use ConnectorIntegrationV2 trait instead of legacy ConnectorIntegration. Using the wrong trait breaks UCS architectural requirements.

**Context / When This Applies:**
This applies to all connector implementations in the UCS connector-service architecture.

**Code Example - WRONG:**
```rust
impl ConnectorIntegration<Authorize, PaymentsAuthorizeData, PaymentsResponseData>
    for MyConnector
{
    // Legacy trait - will not work in UCS
}
```

**Code Example - CORRECT:**
```rust
impl ConnectorIntegrationV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<T>,
    PaymentsResponseData
> for MyConnector<T>
where
    T: PaymentMethodDataTypes + Debug + Sync + Send + 'static + Serialize
{
    // Correct UCS trait
}
```

**Why This Matters:**
- UCS requires V2 traits for enhanced type safety
- V2 includes flow data separation for better architecture
- Required for gRPC integration in UCS
- Wrong trait will cause compilation failures

**How to Fix:**
1. Replace `ConnectorIntegration` with `ConnectorIntegrationV2`
2. Add `PaymentFlowData` as second type parameter
3. Add generic `<T>` to request data type
4. Update connector struct to be generic: `MyConnector<T>`
5. Add trait bounds to implementation
6. Update all method signatures accordingly

**Auto-Fix Rule:**
```
IF file contains "impl ConnectorIntegration<"
AND NOT contains "ConnectorIntegrationV2<"
THEN suggest: "Replace ConnectorIntegration with ConnectorIntegrationV2 and add flow data parameter"
CONFIDENCE: High
```

**Related Patterns:**
- See: guides/patterns/README.md#ucs-architecture
- See: FB-001 (RouterDataV2)
- See: guides/connector_integration_guide.md#trait-implementation

**Lessons Learned:**
Always start with UCS templates which have correct trait implementations. Referencing non-UCS connectors will lead to using wrong patterns.

**Prevention:**
- Use add_connector.sh script for initial scaffolding
- Always reference UCS-specific pattern files
- Run quality checks after each implementation
- Review template files before starting

---
```

### Example 2: Success Pattern

**Complete Example:**

```markdown
### FB-501: Reusable Amount Conversion Utility

**Metadata:**
```yaml
id: FB-501
category: SUCCESS_PATTERN
severity: INFO
connector: stripe
flow: All
date_added: 2024-01-15
status: Active
frequency: 1
impact: High
tags: [best-practice, reusability, transformers, amount-handling]
```

**Issue Description:**
Excellent implementation of reusable amount conversion that properly handles both minor and base currency units.

**Context / When This Applies:**
When implementing amount transformations in connector integrations.

**Code Example - CORRECT:**
```rust
use common_utils::types::{MinorUnit, StringMinorUnit};
use domain_types::utils;

// Use existing utilities instead of recreating
fn convert_amount(
    amount: MinorUnit,
    currency: Currency,
    unit: CurrencyUnit,
) -> CustomResult<String, errors::ConnectorError> {
    match unit {
        CurrencyUnit::Base => utils::to_currency_base_unit(amount, currency),
        CurrencyUnit::Minor => Ok(amount.to_string()),
    }
}
```

**Why This Is Good:**
- Reuses battle-tested utility functions
- Doesn't recreate currency conversion logic
- Handles both unit types correctly
- Type-safe with proper error handling
- Follows DRY principle

**Reusability:**
This pattern can be used in all connector implementations for amount handling.

**Related Patterns:**
- See: common_utils documentation
- See: domain_types::utils module

**Lessons Learned:**
Always check for existing utilities before implementing common functionality. The codebase has robust, tested utilities for common operations like amount conversion.

**Prevention:**
Review common_utils and domain_types modules before implementing transformers to identify reusable functionality.

---
```

### Example 3: Performance Warning

**Complete Example:**

```markdown
### FB-701: Avoid String Allocations in Hot Path

**Metadata:**
```yaml
id: FB-701
category: PERFORMANCE
severity: WARNING
connector: general
flow: All
date_added: 2024-01-15
status: Active
frequency: 2
impact: Medium
tags: [performance, optimization, transformers]
```

**Issue Description:**
Repeated String allocations in transformation logic creates unnecessary performance overhead.

**Context / When This Applies:**
In request/response transformers that are called for every transaction.

**Code Example - WRONG:**
```rust
fn transform_request(data: &RouterData) -> ConnectorRequest {
    ConnectorRequest {
        // Allocating new strings on every call
        reference: format!("REF_{}", data.attempt_id.clone()),
        description: format!("Payment for {}", data.description.clone()),
        // Multiple clones and allocations
    }
}
```

**Code Example - CORRECT:**
```rust
fn transform_request(data: &RouterData) -> ConnectorRequest {
    ConnectorRequest {
        // Borrow when possible, allocate only when necessary
        reference: format!("REF_{}", data.attempt_id),
        description: data.description.as_ref(),
        // Minimize unnecessary clones
    }
}
```

**Why This Matters:**
- Transformers are called on every payment request (hot path)
- Unnecessary allocations impact performance at scale
- Memory pressure increases with high transaction volume
- Simple optimizations have measurable impact

**How to Fix:**
1. Identify fields that can use references instead of owned strings
2. Remove unnecessary .clone() calls
3. Use string references (&str) where possible
4. Only allocate when transformation is actually needed
5. Use Cow<str> for conditionally owned strings

**Related Patterns:**
- See: Rust performance book on string handling
- See: FB-602 (unnecessary clones in general)

**Lessons Learned:**
Profile transformers to identify allocation hotspots. Small optimizations in hot paths compound significantly.

**Prevention:**
- Review transformers for unnecessary allocations
- Prefer borrowing over owning when possible
- Use clippy to identify unnecessary clones
- Benchmark critical paths

---
```

---

## Maintenance

### Updating Frequency Counts

When you observe an existing feedback pattern:

1. Find the feedback entry in `feedback.md`
2. Increment the `frequency` field
3. Update the `date_added` to show latest occurrence (optional)

**Example:**

```yaml
# Before
frequency: 3

# After observing the issue again
frequency: 4
```

### Changing Status

Update the `status` field when:

**Active → Resolved:**
- Issue is fixed across all connectors
- Pattern is no longer observed
- Solution is well-established

**Active → Archived:**
- Pattern becomes obsolete
- UCS architecture changes
- Better solution supersedes it

**Example:**

```yaml
# When issue is resolved
status: Resolved
resolution_date: 2024-02-15
resolution_notes: "All connectors migrated to RouterDataV2"
```

### Archiving Old Feedback

When feedback becomes irrelevant:

1. Update status to "Archived"
2. Add archive reason
3. Move to Section 7: Historical Feedback Archive
4. Keep for historical reference

**Example Format:**

```markdown
### FB-042: [Archived] Old Pattern Name

**Archived:** 2024-03-01
**Reason:** UCS architecture change made this pattern obsolete
**Replacement:** See FB-150 for new pattern

[Original content...]
```

---

## Questions?

If you're unsure about:
- **Which category to use**: Default to CODE_QUALITY and refine later
- **Severity assignment**: Start with WARNING and adjust based on review
- **ID prefix selection**: Use ANTI-XXX for general anti-patterns, or ask in code review
- **Writing style**: Review existing feedback entries as templates

---

**Happy Contributing!** 🎉

Every feedback entry improves the quality of all future connector implementations.
