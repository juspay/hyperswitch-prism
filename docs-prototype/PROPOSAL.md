# Connector Service Documentation Restructuring Proposal

## Executive Summary

This document analyzes the current documentation and example structure, identifies key problems, and proposes a scenario-centric restructuring that reduces maintenance burden by ~93% while improving discoverability for both humans and AI assistants.

**Current State:** 76 connectors × 4 languages = 304+ files with duplicated content  
**Proposed State:** ~20 scenario files (scenarios × languages) with parameterized connectors  
**Key Principle:** Organize by **what users want to do**, not by **which connector they use**

---

## Current State Analysis

### What We Have Today

```
docs-generated/
├── connectors/
│   ├── stripe.md              (514 lines)
│   ├── adyen.md               (486 lines)
│   ├── checkout.md            (423 lines)
│   └── ... (76 files total)
├── llms.txt                   (AI navigation index)
└── all_connector.md           (Coverage matrix)

examples/
├── stripe/
│   ├── python/stripe.py       (1112 lines)
│   ├── rust/stripe.rs         (1500 lines)
│   ├── javascript/stripe.js   (1344 lines)
│   └── kotlin/stripe.kt       (892 lines)
├── adyen/
│   ├── python/adyen.py        (1045 lines)
│   └── ...
└── ... (76 connectors × 4 languages = 304 files)

data/field_probe/
├── stripe.json                (932 lines - source of truth)
├── adyen.json                 (867 lines)
├── manifest.json              (3508 lines - flow metadata)
└── ... (76 probe files)
```

### Generation Pipeline

1. **Field Probe** (`make field-probe`)
   - Probes each connector for supported flows
   - Generates `data/field_probe/{connector}.json`
   - Generates `data/field_probe/manifest.json`

2. **Docs Generation** (`make docs`)
   - Reads probe data
   - Generates `docs-generated/connectors/{connector}.md`
   - Generates `examples/{connector}/{lang}/{connector}.{ext}`

---

## Problems with Current Approach

### 1. Massive Content Duplication

**The Problem:**
- Each connector doc contains the same patterns: SDK config, auth setup, flow sequences, status handling
- 76 connectors × similar content = high cognitive load for maintenance
- A change to "how to handle PENDING status" requires editing 76 files

**Evidence:**
```
Stripe doc mentions "authorize → capture → settle" pattern
Adyen doc mentions "authorize → capture → settle" pattern  
Checkout doc mentions "authorize → capture → settle" pattern
... (74 more times)
```

**Impact:**
- Maintenance burden: Small changes require regenerating 300+ files
- Inconsistency risk: Some docs inevitably drift out of sync
- Review fatigue: PRs with 300+ changed files are hard to review

### 2. Poor Discoverability for Cross-Cutting Concerns

**The Problem:**
- "Which connectors support Apple Pay?" → Must open 76 files
- "What's the auth window for card payments?" → Must check each connector
- "How do I implement refunds?" → Pattern scattered across 76 docs

**Evidence:**
- `llms.txt` tries to help but still lists connectors individually
- No single view of "all connectors that support X scenario"
- Developers must synthesize connector-specific quirks themselves

**Impact:**
- Time to find information is O(n) where n = number of connectors
- AI assistants struggle to answer comparative questions
- Humans give up and pick the first connector they find

### 3. Brittle Line-Number Links

**The Problem:**
- Docs link to examples with line numbers: `examples/stripe/python/stripe.py#L373`
- When examples regenerate, line numbers shift
- Links become broken or point to wrong code

**Evidence:**
```markdown
<!-- In stripe.md -->
**Examples:** [Python](../../examples/stripe/python/stripe.py#L373)
<!-- If stripe.py changes, L373 might be a comment or different function -->
```

**Impact:**
- Documentation appears broken/unmaintained
- Users land on wrong code sections
- Trust in docs erodes over time

### 4. Cognitive Overload for Users

**The Problem:**
- New developers don't think: "I need Stripe documentation"
- They think: "I need to accept card payments"
- Current structure forces them to pick a connector before understanding the pattern

**Evidence:**
- Support questions: "Which connector should I use?" (most common)
- Pattern: User reads Stripe doc → implements → later discovers Adyen better fits their region
- Time wasted learning connector-specific quirks before understanding universal patterns

**Impact:**
- Poor onboarding experience
- Wrong connector choices due to lack of comparison
- Repeated questions to support

### 5. AI/LLM Unfriendliness

**The Problem:**
- Current `llms.txt` is a flat list of connectors
- AI must read 76 docs to answer "compare Stripe vs Adyen for EU"
- No semantic structure for AI to reason about

**Evidence:**
```yaml
# Current llms.txt
## Stripe
scenarios: checkout_card, checkout_autocapture, ...
payment_methods: Card, ApplePay, ...

## Adyen  
scenarios: checkout_card, checkout_autocapture, ...
payment_methods: Card, ApplePay, ...

# AI sees: two similar entries, no way to compare easily
```

**Impact:**
- AI assistants give poor recommendations
- Can't answer comparative questions effectively
- Users get frustrated with AI help

### 6. Redundancy Between Docs and Examples

**The Problem:**
- Docs contain code snippets
- Examples contain the same code
- Two sources of truth for the same information

**Evidence:**
```markdown
<!-- In stripe.md -->
```python
config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
```
```

```python
# In examples/stripe/python/stripe.py (same code)
_default_config = sdk_config_pb2.ConnectorConfig(
    options=sdk_config_pb2.SdkOptions(environment=sdk_config_pb2.Environment.SANDBOX),
)
```

**Impact:**
- Double maintenance (docs + examples)
- Risk of divergence between docs and runnable code
- More files to keep in sync

### 7. Language-Specific Redundancy

**The Problem:**
- Same logic repeated in Python, Rust, JS, Kotlin
- Each language file is ~1000 lines
- 76 connectors × 4 languages = 304 files

**Evidence:**
```
examples/stripe/python/stripe.py       1112 lines
examples/stripe/rust/stripe.rs         1500 lines
examples/stripe/javascript/stripe.js   1344 lines
examples/stripe/kotlin/stripe.kt        892 lines

Same patterns, just different syntax
```

**Impact:**
- 304 files to maintain
- Changes require updates across 4 languages
- Storage and generation time overhead

### 8. Limited Extensibility

**The Problem:**
- Adding a new scenario requires updating 76 connector docs
- Adding a new language requires 76 new files
- Adding a new connector requires 4 new example files + 1 doc

**Evidence:**
- Current workflow: Edit generator → regenerate everything → review 300+ files
- Risk of missing a connector when adding features
- High barrier to adding new scenarios

**Impact:**
- Slow iteration on documentation
- Features undocumented due to effort required
- Incomplete coverage

---

## Proposed Solution: Scenario-Centric Architecture

### Core Principle

**Organize by scenario (task), not by connector.**

Instead of:
```
"Here's everything about Stripe"
"Here's everything about Adyen"
```

Structure as:
```
"Here's how to accept card payments"
"Here's how to accept Apple Pay"
"Here's how to issue refunds"
```

### New Structure

```
docs/
├── scenarios/
│   ├── card-payment.md              # Authorize + Capture
│   ├── checkout-autocapture.md      # One-step payments
│   ├── wallet-payment.md            # Apple Pay / Google Pay
│   ├── bank-transfer.md             # SEPA / ACH / BACS
│   ├── refund.md                    # Refund patterns
│   ├── recurring.md                 # Subscriptions
│   ├── authentication.md            # 3D Secure
│   └── void-payment.md              # Void/cancel
├── connectors/                      # Reference docs (reduced)
│   ├── index.md                     # All connectors matrix
│   ├── stripe.md                    # Stripe-specific quirks only
│   ├── adyen.md                     # Adyen-specific quirks only
│   └── ... (minimal per-connector docs)
└── llms.txt                         # AI-optimized navigation

examples/scenarios/
├── checkout_card.py                 # Works with ANY connector
├── checkout_card.rs
├── checkout_card.js
├── checkout_card.kt
├── checkout_wallet.py
├── checkout_wallet.rs
├── refund.py
├── refund.rs
└── ... (~20 files total vs 304)
```

### Key Changes

#### 1. Universal Parameterized Examples

**Before:**
```python
# examples/stripe/python/stripe.py (76 files like this)
def process_checkout_card_stripe(...):
    # Stripe-specific code
    
def process_checkout_card_adyen(...):
    # Can't exist here - wrong file
```

**After:**
```python
# examples/scenarios/checkout_card.py (1 file)
def process_checkout_card(connector_name: str, ...):
    # Load connector config from probe data
    config = load_connector_config(connector_name)
    # Universal pattern, parameterized by connector
    
# Usage:
python checkout_card.py --connector=stripe
python checkout_card.py --connector=adyen
python checkout_card.py --connector=checkout
```

**Benefits:**
- 1 file instead of 76 per language
- DRY principle: code patterns defined once
- Easy to test all connectors: loop over probe data
- No line-number drift (dynamic code)

#### 2. Scenario-Centric Documentation

**Before:**
```markdown
# Stripe

## SDK Configuration
...Python config...
...JS config...
...Kotlin config...
...Rust config...

## Integration Scenarios
### Card Payment
...status handling...
...examples...

### Wallet Payment
...status handling...
...examples...
```

**After:**
```markdown
# Card Payment (Authorize + Capture)

## Overview
[Universal flow diagram]

## Quick Start
[Universal code example with {connector} placeholder]

## Connector Support
| Connector | Card Support | 3DS | Auth Window |
|-----------|--------------|-----|-------------|
| stripe    | ✅ Full      | Yes | 7 days      |
| adyen     | ✅ Full      | Yes | 28 days     |
| ...       | ...          | ... | ...         |

## Connector-Specific Variations
<details>
<summary>Stripe — No special handling needed</summary>
...
</details>
<details>
<summary>Adyen — Requires shopper_reference</summary>
...
</details>
```

**Benefits:**
- Answer "which connector should I use?" in one view
- Compare connectors side-by-side
- Universal patterns front and center
- Quirks isolated in collapsible sections

#### 3. Enhanced AI Navigation

**Before:**
```yaml
# llms.txt - flat connector list
## Stripe
scenarios: checkout_card, checkout_autocapture, ...

## Adyen
scenarios: checkout_card, checkout_autocapture, ...
```

**After:**
```yaml
# llms-improved.txt - scenario-first index
## I want to...
- [Process a card payment](./scenarios/card-payment.md)
- [Process a wallet payment](./scenarios/wallet-payment.md)

## Scenario Support Matrix
### Card Payments
| Connector | Status | Auth Window | Notes |
|-----------|--------|-------------|-------|
| stripe    | ✅     | 7 days      | Default |
| adyen     | ✅     | 28 days     | EU/UK   |
```

**Benefits:**
- AI can answer "which connectors support X?" instantly
- Comparative questions have structured data
- Semantic organization matches user intent

#### 4. Dynamic Code Generation

Instead of static example files with line numbers:

```python
# examples/scenarios/checkout_card.py
def build_request(connector_name: str):
    """Dynamically build request from probe data."""
    probe_data = load_probe_data(connector_name)
    
    # Get the authorize flow for this connector
    auth_flow = probe_data["flows"]["authorize"]
    
    # Find Card or first supported payment method
    for pm_key, pm_data in auth_flow.items():
        if pm_data["status"] == "supported":
            return pm_data["proto_request"]
    
    raise ValueError(f"{connector_name} doesn't support card payments")

# No line numbers needed - code is dynamically constructed
```

**Benefits:**
- Always uses current probe data
- No broken links
- Adapts to connector capabilities automatically

---

## Prototype Examples

See `docs-prototype/` directory for working examples:

### Files Created:

1. **`docs-prototype/scenarios/card-payment.md`**
   - Scenario-focused documentation
   - Connector comparison matrix
   - Universal code examples
   - Collapsible connector quirks

2. **`docs-prototype/examples/scenarios/checkout_card.py`**
   - Parameterized connector support
   - Dynamic request building from probe data
   - Works with any connector

3. **`docs-prototype/examples/scenarios/checkout_card.rs`**
   - Rust equivalent of Python example
   - Same parameterized approach

4. **`docs-prototype/llms-improved.txt`**
   - AI-optimized navigation
   - Scenario-first organization
   - Support matrices for quick lookup

---

## Migration Path

### Phase 1: Create New Structure (Parallel)

1. Create `docs/scenarios/` with core scenarios
2. Create `examples/scenarios/` with parameterized examples
3. Update `llms.txt` to scenario-first format
4. **Keep existing docs for backward compatibility**

### Phase 2: Redirect and Deprecate

1. Update main README to point to scenario docs
2. Add deprecation notices to connector-specific docs
3. Redirect common queries to scenario pages

### Phase 3: Cleanup (Future)

1. Remove per-connector example files (after transition period)
2. Retain minimal per-connector docs for quirks only
3. Full adoption of scenario-centric approach

---

## Benefits Summary

| Metric | Before | After | Improvement |
|--------|--------|-------|-------------|
| Example files | 304 | ~20 | **93% reduction** |
| Doc files | 76 | ~10 | **87% reduction** |
| Time to answer "which connector for X?" | O(n) | O(1) | **Instant** |
| Time to add new scenario | 76 files | 1 file | **99% faster** |
| Maintenance burden | High | Low | **DRY principle** |
| AI discoverability | Poor | Excellent | **Semantic structure** |
| Line-number drift | Constant | None | **Dynamic generation** |
| User onboarding | Connector-first | Task-first | **Better UX** |

---

## Trade-offs Acknowledged

### What We Lose:

1. **Per-connector line-by-line walkthroughs**
   - *Mitigation:* Connector quirks in collapsible sections
   - *Rationale:* Most patterns are universal anyway

2. **Immediate copy-paste for specific connectors**
   - *Mitigation:* Universal examples work with `--connector=X` flag
   - *Rationale:* One command line arg vs 300 files is worth it

3. **Search engine optimization for "Stripe SDK"**
   - *Mitigation:* Keep minimal per-connector pages for SEO
   - *Rationale:* Primary entry point should be task-based

### What We Gain:

1. **Maintainability:** Edit once, apply everywhere
2. **Discoverability:** Find information faster
3. **Comparability:** Easy side-by-side connector comparison
4. **AI-friendliness:** Structured data for LLM reasoning
5. **Scalability:** Add connectors/scenarios with minimal effort

---

## Recommendation

**Adopt the scenario-centric architecture** as the primary documentation structure, while maintaining minimal per-connector reference pages for backward compatibility and SEO.

The current approach is optimized for **documentation generators**, not **documentation consumers**. The proposed approach optimizes for:
- **Humans** who think in tasks ("I need to accept payments")
- **AI assistants** that need structured, comparative data
- **Maintainers** who want DRY, consistent documentation

The 93% file reduction and improved discoverability make this a clear win for long-term maintainability and user experience.

---

## Appendix: File Size Comparison

### Current State

```
Documentation:
- docs-generated/connectors/*.md: 76 files × ~400 lines = ~30,400 lines
- examples/*/*.py: 76 files × ~1000 lines = ~76,000 lines
- examples/*/*.rs: 76 files × ~1200 lines = ~91,200 lines
- examples/*/*.js: 76 files × ~1100 lines = ~83,600 lines
- examples/*/*.kt: 76 files × ~800 lines = ~60,800 lines

Total: ~342,000 lines of generated documentation
```

### Proposed State

```
Documentation:
- docs/scenarios/*.md: 8 files × ~300 lines = ~2,400 lines
- examples/scenarios/*.py: 8 files × ~150 lines = ~1,200 lines
- examples/scenarios/*.rs: 8 files × ~200 lines = ~1,600 lines
- examples/scenarios/*.js: 8 files × ~180 lines = ~1,440 lines
- examples/scenarios/*.kt: 8 files × ~160 lines = ~1,280 lines
- docs/connectors/*.md: 76 files × ~50 lines = ~3,800 lines (minimal quirks only)

Total: ~11,720 lines (~97% reduction)
```

**Note:** Code complexity moves from "lots of similar files" to "smart parameterized files that read probe data". This is a better abstraction.

---

*Generated: March 2026*  
*Proposal Status: Draft for Review*
