# Quality Review Report Template

> **Purpose:** Standalone template for Quality Guardian Subagent to use when conducting code quality reviews.
> **Note:** This is a reference copy. The same template appears at the top of `feedback.md`.

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

# Usage Instructions

## For Quality Guardian Subagent

### 1. Pre-Review Preparation
```bash
# Read the feedback database
Read: guides/feedback.md

# Identify relevant patterns for current flow
Extract: Flow-specific patterns
Extract: UCS critical patterns
Extract: Common anti-patterns

# Prepare checklist
Create: Custom checklist for this review
```

### 2. Code Analysis
```bash
# Analyze modified files
Files to review:
- crates/integrations/connector-integration/src/connectors/[connector_name].rs
- crates/integrations/connector-integration/src/connectors/[connector_name]/transformers.rs

# Check for patterns
For each file:
    - Scan for UCS pattern violations
    - Check Rust best practices
    - Validate connector patterns
    - Assess code quality
    - Review error handling
```

### 3. Quality Scoring
```bash
# Count issues by severity
critical_count = [count critical issues]
warning_count = [count warning issues]
suggestion_count = [count suggestion issues]

# Calculate score
quality_score = 100 - (critical_count × 20) - (warning_count × 5) - (suggestion_count × 1)

# Determine status
if quality_score < 60:
    status = "BLOCKED"
elif quality_score < 80:
    status = "PASS WITH WARNINGS"
else:
    status = "PASS"
```

### 4. Report Generation
```bash
# Fill in this template
- Replace all [placeholders]
- Add specific code examples
- Provide actionable fixes
- Include references to feedback.md entries
```

### 5. Knowledge Base Update
```bash
# Update feedback.md if needed
if new_pattern_discovered:
    Add new feedback entry to feedback.md

if existing_pattern_observed:
    Increment frequency count for FB-XXX
```

### 6. Decision Making
```bash
if status == "BLOCKED":
    - Provide detailed fix instructions
    - Do not proceed to next flow
    - Wait for fixes and re-review

if status == "PASS WITH WARNINGS":
    - Document warnings
    - Allow progression
    - Recommend fixes

if status == "PASS":
    - Approve progression
    - Document success patterns
    - Update metrics
```

## Review Checklist by Phase

### Foundation Setup Review
- [ ] Connector struct uses generic `<T: PaymentMethodDataTypes>`
- [ ] ConnectorCommon trait properly implemented
- [ ] Authentication type structure correct
- [ ] Error response structure defined
- [ ] UCS imports used (domain_types, not hyperswitch_*)
- [ ] Base URL and currency unit configured
- [ ] Build succeeds

### Per-Flow Review (Authorize, Capture, Void, Refund, PSync, RSync)
- [ ] ConnectorIntegrationV2 trait used (not ConnectorIntegration)
- [ ] RouterDataV2 used throughout (not RouterData)
- [ ] Correct generic type parameters
- [ ] Request transformer complete and correct
- [ ] Response transformer complete and correct
- [ ] Status mapping comprehensive
- [ ] Error handling proper
- [ ] Payment method support adequate
- [ ] Follows pattern file (guides/patterns/pattern_[flow].md)
- [ ] No code duplication
- [ ] Edge cases considered
- [ ] Build succeeds

### Final Comprehensive Review
- [ ] All flows consistent
- [ ] Cross-flow patterns coherent
- [ ] Overall code quality high
- [ ] Documentation adequate
- [ ] Performance considerations addressed
- [ ] Security reviewed
- [ ] UCS compliance score ≥ 80%

## Quality Scoring Reference

### Score Ranges

**95-100: Excellent ✨**
- 0-1 suggestions only
- No warnings or critical
- Exemplary implementation
- Auto-approve, document success patterns

**80-94: Good ✅**
- Minor warnings or a few suggestions
- No critical issues
- Approve with notes
- May suggest optional improvements

**60-79: Fair ⚠️**
- Multiple warnings or many suggestions
- No critical issues
- Approve with significant warnings
- Recommend fixes before next phase

**40-59: Poor ❌**
- One or more critical issues
- Or many warnings
- Block until critical fixes applied
- Provide detailed fix instructions

**0-39: Critical 🚨**
- Multiple critical issues
- Fundamental problems
- Immediate block
- May require significant rework

### Severity Scoring

| Severity | Score Impact | Example Count | Total Impact |
|----------|--------------|---------------|--------------|
| Critical | -20 points   | 2 issues      | -40 points   |
| Warning  | -5 points    | 3 issues      | -15 points   |
| Suggestion | -1 point   | 5 issues      | -5 points    |
| **Total** | **-60 points** | **10 issues** | **Score: 40** |

## Example Quality Reports

### Example 1: Excellent Implementation (Score: 98)

**Complete Example:**

```markdown
## Quality Review Report: Stripe - Authorize Flow

**Review Date:** 2024-01-15
**Reviewer:** Quality Guardian Subagent
**Phase:** Authorize

---

### 🎯 Overall Quality Score: 98/100

**Status:** ✅ PASS (Excellent)

---

### 📊 Issue Summary

| Severity | Count | Impact on Score |
|----------|-------|-----------------|
| 🚨 Critical | 0 | 0 |
| ⚠️ Warning | 0 | 0 |
| 💡 Suggestion | 2 | -2 |

---

### 💡 Suggestions (Nice to Have) - Count: 2

#### SUGGESTION-1: Add Documentation for Complex Transformer

**Location:** `transformers.rs:45`

**Suggestion:**
Add a comment explaining the currency conversion logic.

**Benefit:**
Improves maintainability for future developers.

#### SUGGESTION-2: Extract Common URL Building Pattern

**Location:** `stripe.rs:120`

**Suggestion:**
Extract URL building to a helper function for reusability.

**Benefit:**
Reduces duplication if more endpoints are added.

---

### ✨ Success Patterns Observed - Count: 3

#### SUCCESS-1: Excellent Error Handling
All error scenarios properly mapped to UCS error types.

#### SUCCESS-2: Clean Transformer Structure
Request/response transformers are clear and maintainable.

#### SUCCESS-3: Comprehensive Status Mapping
All connector statuses mapped to appropriate UCS statuses.

---

### 🎯 Decision & Next Steps

**Decision:** ✅ APPROVE TO PROCEED

Excellent implementation! Proceed to PSync flow.

**Optional Actions (Recommended):**
1. Consider adding documentation for complex transformations
2. Extract reusable patterns

---
```

### Example 2: Blocked Implementation (Score: 35)

**Complete Example:**

```markdown
## Quality Review Report: ExampleConnector - Authorize Flow

**Review Date:** 2024-01-15
**Reviewer:** Quality Guardian Subagent
**Phase:** Authorize

---

### 🎯 Overall Quality Score: 35/100

**Status:** ❌ BLOCKED (Critical Issues)

---

### 📊 Issue Summary

| Severity | Count | Impact on Score |
|----------|-------|-----------------|
| 🚨 Critical | 3 | -60 |
| ⚠️ Warning | 1 | -5 |
| 💡 Suggestion | 0 | 0 |

---

### 🚨 Critical Issues (Must Fix Before Proceeding) - Count: 3

#### CRITICAL-1: Using RouterData Instead of RouterDataV2

**Feedback ID:** FB-001
**Category:** UCS_PATTERN_VIOLATION
**Location:** `example_connector.rs:25`

**Problem:**
Implementation uses legacy `RouterData` type instead of UCS-required `RouterDataV2`.

**Current Code:**
```rust
impl ConnectorIntegration<Authorize, RouterData<...>> {
    // ...
}
```

**Required Fix:**
```rust
impl ConnectorIntegrationV2<
    Authorize,
    PaymentFlowData,
    PaymentsAuthorizeData<T>,
    PaymentsResponseData
> for ExampleConnector<T> {
    // ...
}
```

**Why This Is Critical:**
Breaks UCS architectural requirements. Will cause integration failures.

**Estimated Fix Time:** 15 minutes

---

[Additional critical issues...]

---

### 🎯 Decision & Next Steps

**Decision:** ❌ BLOCK UNTIL FIXES APPLIED

**Blocking Justification:**
Multiple critical UCS pattern violations prevent proceeding.

**Required Actions:**
1. Replace RouterData with RouterDataV2 in example_connector.rs:25
2. Update imports to use domain_types in example_connector.rs:1-10
3. Add generic type parameter to struct in example_connector.rs:15

**Estimated Total Fix Time:** 30 minutes

---
```

---

**End of Template Documentation**
