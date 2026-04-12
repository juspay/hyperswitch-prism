# Documentation Strategy


This document outlines the documentation structure, organization, and strategy for the Unified Prism (UCS) project.


## Vision


Become the **"Linux for Payments"** — a universal standard that makes payment processor integration as simple as writing SQL queries. Our documentation must reflect this vision by providing clear, actionable guidance that turns months of integration work into hours.


## Overview


Documentation is split into two distinct folders within the juspay/connector-service repo:
- **`/docs`** - Handwritten, curated content that explains concepts, provides guidance, and offers insights
- **`/docs-generated`** - Auto-generated from code (API references, type definitions, SDK documentation)


This separation enables independent CI/CD pipelines, version control strategies, and publishing workflows while presenting a unified experience to readers.


---


## Documentation Folder Structure


### `/docs` - Handwritten Documentation


Curated, manually written content that provides context, explanations, and guidance.


```
docs/
├── README.md                     # Documentation landing page
├── strategy.md                   # This file
│
├── getting-started/              # Getting started guides
│   ├── quickstart.md
│   ├── installation.md
│   ├── first-payment.md          # Authorize with error handling across all languages showing user-friendly error messages
│   └── extending-to-more-flows.md
│
├── architecture/                 # System architecture documentation
│   ├── overview.md
│   ├── core-components.md
│   ├── connectors-services-subservices-methods.md  # Hierarchy, composition & granularity
│   ├── specs-and-dsl.md          # Why proto? Why domain types? DSL for FFI, DSL for
│   │                             # gRPC service? What problems it solves for developers
│   ├── id-and-object-modelling.md
│   ├── connector-settings-and-overrides.md
│   ├── environment-settings.md   # Feature control
│   ├── testing-framework.md
│   ├── library-modes-of-usage.md
│   ├── error-mapping.md          # How we unify errors
│   ├── error-handling.md         # How we handle errors
│   ├── money.md
│   ├── integrity.md
│   ├── source-verification.md
│   ├── sdk-generation.md
│   ├── docs-generation.md
│   ├── test-generation.md
│   ├── code-generation.md
│   ├── compliance.md
│   ├── glossary.md
│   └── versioning.md
│
├── audit-trail/                  # Debug and observability
│   ├── understanding-transformations.md
│   ├── reading-request-logs.md
│   └── debugging-failures.md
│
└── blog/                         # Blog posts and articles
   ├── why-we-built-ucs.md
   ├── why-llms-cannot-vibe-code-payments-for-production-use.md
   └── universal-grammar-for-payments.md
       # A Universal Grammar for Payments — How Rust's Trait System,
       # Types, and Macros Turned Fragmented Payment APIs into a
       # Deterministic Language
```


### `/docs-generated` - Generated Documentation


Auto-generated content from proto definitions, code annotations, and connector configurations.


```
docs-generated/
│
├── sdks/                         # SDK documentation per language
│   ├── node/
│   ├── java/
│   ├── python/
│   └── rust/
│
├── api-reference/                # API endpoint documentation
│   ├── domain-schema/
│   └── services/
│
└── connectors/                   # Connector-specific reference
   ├── overview.md
   ├── stripe.md
   ├── adyen.md
   ├── braintree.md
   └── ...



```



---
## Core Principles


### 1. Error Handling is Our Biggest Differentiator


Error unification and handling documentation is **priority #1**. Like the Rust compiler, our docs must provide:
- Clear explanation of what went wrong
- Why it happened
- How to fix it with links to relevant connector documentation (Stripe, Adyen, etc.)


**Current problem**: Generic errors like "address not found"
**Target**: Actionable errors with context and remediation steps


### 2. Curated Quality Over LLM-Generated Volume


The value of UCS documentation is in **production-tested, manually curated content** — not purely LLM-generated text. Generated docs provide accuracy; handwritten docs provide wisdom.


### 3. Proto Definitions Are Source of Truth


All generated documentation derives from proto definitions. Never manually edit generated files — changes will be overwritten on the next sync.


---


## CI/CD Integration with juspay/hyperswitch-docs


To present a unified documentation experience on GitBook, we use a **merged sync strategy** that combines content from both `/docs` and `/docs-generated` into a single `ucs/` folder in the `juspay/hyperswitch-docs` repository.


### Architecture


```
┌─────────────────────────────────┐         ┌──────────────────────────┐
│  connector-service              │         │  juspay/hyperswitch-docs │
│  (this repo)                    │         │  (docs repo)             │
│                                 │         │                          │
│  ┌──────────────────┐           │         │  ┌────────────────────┐  │
│  │ /docs            │           │  Sync   │  │ /ucs/              │  │
│  │ (handwritten)    │───────────┼────────▶│  │ (merged content)   │  │
│  │                  │           │         │  │                    │  │
│  │ /docs-generated  │           │         │  │ • getting-started/ │  │
│  │ (generated)      │───────────┘         │  │ • architecture/    │  │
│  └──────────────────┘                     │  │ • api-reference/   │  │
│                                           │  │ • sdks/            │  │
│                                           │  │ • connectors/      │  │
│                                           │  │ • ...              │  │
│                                           │  └────────────────────┘  │
│                                           │                          │
│                                           │  GitBook Integration     │
└───────────────────────────────────────────┘                          │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```


### Unified CI/CD Pipeline


A single workflow handles both handwritten and generated docs, merging them in the correct order:


```yaml
# .github/workflows/sync-docs.yml
name: Sync Docs to GitBook


on:
 push:
   branches: [main]
   paths:
     - 'docs/**'
     - 'crates/types-traits/grpc-api-types/**'
     - 'crates/integrations/connector-integration/**'
     - 'sdk/**'
 release:
   types: [published]
 workflow_dispatch:  # Allow manual trigger


jobs:
 sync:
   runs-on: ubuntu-latest
   steps:
     - name: Checkout connector-service
       uses: actions/checkout@v4


     - name: Generate documentation
       run: |
         # Generate docs from proto files and code
         make generate-docs
         make generate-sdk-docs
         make generate-connector-docs


     - name: Sync to hyperswitch-docs
       run: |
         git clone https://github.com/juspay/hyperswitch-docs.git


         # Clear existing UCS docs
         rm -rf hyperswitch-docs/ucs/**


         # Create UCS docs directory
         mkdir -p hyperswitch-docs/ucs


         # Step 1: Copy handwritten docs first (base structure)
         cp -r docs/** hyperswitch-docs/ucs/


         # Step 2: Merge generated docs into corresponding sections
         # Generated API reference overwrites/extends handwritten
         if [ -d "docs-generated/api-reference" ]; then
           cp -r docs-generated/api-reference/* hyperswitch-docs/ucs/api-reference/
         fi


         # Generated SDKs
         if [ -d "docs-generated/sdks" ]; then
           mkdir -p hyperswitch-docs/ucs/sdks
           cp -r docs-generated/sdks/* hyperswitch-docs/ucs/sdks/
         fi


         # Generated connector reference
         if [ -d "docs-generated/connectors" ]; then
           mkdir -p hyperswitch-docs/ucs/connectors
           cp -r docs-generated/connectors/* hyperswitch-docs/ucs/connectors/
         fi


         # Copy generated SUMMARY.md if it exists
         if [ -f "docs-generated/SUMMARY.md" ]; then
           cp docs-generated/SUMMARY.md hyperswitch-docs/ucs/SUMMARY.md
         fi


         # Configure git and commit
         cd hyperswitch-docs
         git config user.name "UCS Docs Bot"
         git config user.email "ucs-docs@juspay.in"
         git add ucs/


         # Only commit if there are changes
         if git diff --cached --quiet; then
           echo "No changes to sync"
           exit 0
         fi


         git commit -m "Sync UCS docs from connector-service@$GITHUB_SHA"
         git push
```


**Characteristics:**
- **Trigger**: On changes to `/docs`, protos, connectors, or SDKs; also on releases
- **Frequency**: As needed (human-paced for docs, machine-paced for generated)
- **Review**: Required via PR for handwritten changes; automated for generated
- **Rollback**: Manual via git revert or regenerate from previous commit


### Merged GitBook Structure


Content from both sources is merged into a unified structure in `juspay/hyperswitch-docs/ucs/`:


```
hyperswitch-docs/
├── README.md
├── SUMMARY.md                    # GitBook table of contents
│
├── getting-started/              # Cross-project getting started
├── architecture/                 # Cross-project architecture
│
└── ucs/                          # Prism docs (merged)
   ├── README.md                 # UCS landing page
   ├── SUMMARY.md                # UCS-specific navigation
   │
   ├── getting-started/          # Handwritten from /docs
   │   ├── README.md
   │   ├── quick-start.md
   │   ├── installation.md
   │   └── concepts.md
   │
   ├── architecture/             # Handwritten from /docs
   │   ├── README.md
   │   └── overview.md
   │
   ├── api-reference/            # Merged: base from /docs, generated overlays
   │   ├── README.md
   │   ├── domain-schema/        # From /docs
   │   └── services/             # From /docs (populated via generation)
   │       ├── payment-service/
   │       ├── customer-service/
   │       └── ...
   │
   ├── sdks/                     # Merged
   │   ├── README.md             # From /docs
   │   ├── nodejs/               # From /docs
   │   └── python/               # From /docs
   │   # Generated SDK docs added here
   │
   ├── connectors/               # Merged
   │   ├── README.md             # Handwritten from /docs
   │   ├── stripe.md             # Handwritten from /docs
   │   ├── adyen.md              # Handwritten from /docs
   │   └── ...                   # Generated connector specs
   │
   └── rules/                    # From /docs
       ├── README.md
       └── rules.md
```


### Merge Strategy


When syncing, content is merged in this order:


1. **Handwritten base** (`/docs`): All files copied first as the foundation
2. **Generated overlay** (`/docs-generated`): Specific files/directories overwrite or extend:
  - `api-reference/services/` - Auto-generated from proto definitions
  - `sdks/<language>/reference/` - Auto-generated SDK reference docs
  - `connectors/<name>-spec.md` - Auto-generated connector specifications
  - `SUMMARY.md` - Generated navigation (optional)


### Benefits of This Approach


1. **Unified Reader Experience**
  - Seamless navigation between conceptual and reference docs
  - No distinction between "handwritten" and "generated" in the UI
  - Single URL structure for all UCS documentation


2. **Clear Source of Truth per Section**
  - Conceptual docs: Writers own quality in `/docs`
  - Reference docs: Engineering owns accuracy via generation
  - Both sources contribute to the same published structure


3. **Flexible Generation**
  - Generated content can extend existing sections
  - New connectors/SDKs automatically appear in correct locations
  - Handwritten content can link to generated content reliably


4. **Simple Rollback**
  - Single commit per sync makes reverting straightforward
  - Regenerate docs for any historical release


### GitBook Configuration


In `hyperswitch-docs/SUMMARY.md` (maintained in connector-service, synced to docs repo):


```markdown
# Summary


## Getting Started
* [Overview](ucs/getting-started/README.md)
* [Quick Start](ucs/getting-started/quick-start.md)
* [Installation](ucs/getting-started/installation.md)
* [Concepts](ucs/getting-started/concepts.md)


## Architecture
* [Overview](ucs/architecture/overview.md)
* [Core Components](ucs/architecture/README.md)


## API Reference
* [Overview](ucs/api-reference/README.md)
* [Domain Schema](ucs/api-reference/domain-schema/README.md)
* [Services](ucs/api-reference/services/README.md)
 * [Payment Service](ucs/api-reference/services/payment-service/README.md)
 * [Customer Service](ucs/api-reference/services/customer-service/README.md)
 * [Refund Service](ucs/api-reference/services/refund-service/README.md)
 * [Dispute Service](ucs/api-reference/services/dispute-service/README.md)
 * [Event Service](ucs/api-reference/services/event-service/README.md)


## SDKs
* [Overview](ucs/sdks/README.md)
* [Node.js](ucs/sdks/nodejs/README.md)
* [Python](ucs/sdks/python/README.md)


## Connectors
* [Overview](ucs/connectors/README.md)
* [Stripe](ucs/connectors/stripe.md)
* [Adyen](ucs/connectors/adyen.md)
* [Cybersource](ucs/connectors/cybersource.md)


## Rules
* [Overview](ucs/rules/README.md)
* [Rules Reference](ucs/rules/rules.md)


## Blog
* [Why We Built UCS](ucs/blog/why-we-built-ucs.md)
* [Universal Grammar for Payments](ucs/blog/universal-grammar-for-payments.md)
```


---


## Key Principles


### Handwritten Content (`/docs`)
- **Error-first**: Prioritize troubleshooting and error handling docs
- **Actionable**: Every error explanation includes "how to fix"
- **Curated quality**: Production-tested, not LLM-generated
- **Audit-friendly**: Show transformations for debugging
- **Comprehensive architecture**: Document hierarchy, DSL, and design decisions
- **Explain the "why"**: Generated docs tell you "what"; handwritten docs explain "why"


### Generated Content (`/docs-generated`)
- **Single source of truth**: Generated from proto definitions
- **Always accurate**: Never out of sync with implementation
- **Complete coverage**: Every public API, type, and method
- **Never edit manually**: Changes overwritten on regeneration


---


## Maintenance Strategy


### Handwritten (`/docs`)
- Review quarterly for accuracy
- Update within 1 week of feature releases
- Error handling docs updated immediately when new error patterns discovered
- Architecture docs updated when system structure changes
- Community contributions welcome via PRs


### Generated (`/docs-generated`)
- Regenerated on every relevant code change or release
- CI/CD pipeline handles generation and publishing
- No manual edits (changes are overwritten)
- Optionally gitignored in source repo (only synced to docs repo)


---