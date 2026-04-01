# GRACE Framework Enhancement & Integration Prompt
## Optimized for Claude Code Execution

---

## PHASE 1: GRACE REPOSITORY ANALYSIS & DISCOVERY
**Execution Mode: Sequential Discovery with Output Logging**

### Task 1.1: Repository Structure Mapping
```
EXECUTE: Clone/navigate to grace repo (provide path or repo URL)
ANALYZE: Map complete directory structure
OUTPUT: Generate structure.txt with:
  - Root directories and their purposes
  - Key configuration files location
  - Workflow orchestration files (1_orchestrator.md, 2_*, 3_*, 4_*)
  - Connector implementations directory
  - Dependencies and requirements files
VALIDATE: Confirm presence of grace/workflow/ directory
```

### Task 1.2: Grace Core Documentation Analysis
```
EXECUTE: Read and parse the following files sequentially:
  1. grace/README.md (overview)
  2. grace/SETUP.md or INSTALLATION.md (setup instructions)
  3. grace/ARCHITECTURE.md or similar (system design)
  4. grace/API.md or COMMANDS.md (CLI/API reference)
  5. grace/workflow/*.md files (all orchestrator files)

EXTRACT & LOG:
  - How Grace works (system flow diagram in text format)
  - Complete setup procedure with all prerequisites
  - All available commands with parameters and use cases
  - Code implementation patterns used by Grace
  - Integration points with Claude Code
  - Activation mechanism and triggers

OUTPUT: analysis_report.md with structured sections
```

### Task 1.3: Grace & Claude Code Integration Mapping
```
EXECUTE: Search grace repo for:
  - Claude API calls or SDK usage (grep patterns: "claude", "anthropic", "model=")
  - Prompt templates (*.prompt files or prompt strings in code)
  - Claude Code invocations (how Grace triggers code execution)
  - Streaming/response handling patterns
  - Error handling for Claude responses

EXTRACT:
  - List all Claude prompts with context of use
  - Identify prompt templates and variables
  - Map Claude Code activation triggers
  - Document response processing pipeline

OUTPUT: claude_integration_map.md with:
  - Prompt catalog with full text
  - Integration points diagram (text format)
  - Activation flow chart
```

### Task 1.4: Orchestrator Files Deep Dive
```
EXECUTE: Read and compare:
  - grace/workflow/1_orchestrator.md (reference standard)
  - grace/workflow/2_orchestrator.md (identify patterns)
  - grace/workflow/3_orchestrator.md (identify deviations & gaps)
  - grace/workflow/4_orchestrator.md (identify deviations & gaps)

ANALYZE FOR EACH FILE:
  - Structure and format consistency
  - Task definitions and sequencing
  - Connector specifications
  - Prompt injection points
  - Queue management references
  - Failure handling procedures

OUTPUT: orchestrator_comparison.md with:
  - Side-by-side feature comparison table
  - Missing features in 3_* and 4_* vs 1_*
  - Inconsistencies and recommendations
  - Prompt placeholder locations
```

---

## PHASE 2: QUEUE SYSTEM ANALYSIS & ENHANCEMENT
**Execution Mode: Code Investigation with Architectural Recommendations**

### Task 2.1: Existing Queue System Discovery
```
EXECUTE: Search grace repo for queue implementation:
  - Files containing "queue", "task", "worker", "job", "batch"
  - Technology stack: Redis, RabbitMQ, Bull, Celery, or custom?
  - Current queue logic and flow
  - Task prioritization mechanism
  - Failure/retry handling

EXTRACT:
  - Current queue system architecture
  - Task lifecycle and states
  - Processing rules and constraints
  - Scalability patterns

OUTPUT: queue_system_analysis.md with:
  - Current architecture diagram (text/ASCII)
  - Technical stack used
  - Task flow visualization
  - Identified limitations
```

### Task 2.2: Tech Spec Completion Detection
```
ANALYZE: How does Grace detect "tech spec completion"?
  - Look for status checks, completion flags
  - Understand connector-by-connector spec flow
  - Find where queue should trigger (post-spec phase)

DOCUMENT:
  - Current completion detection mechanism
  - Where queue integration should happen
  - Handoff point from spec → queue

OUTPUT: spec_completion_flow.md
```

---

## PHASE 3: PROMPT INJECTION FRAMEWORK DESIGN
**Execution Mode: Architecture Design with Implementation Template**

### Task 3.1: Prompt Storage & Management Strategy
```
DECIDE: Optimal prompt delivery mechanism

COMPARE OPTIONS:
  A) .md File Approach
     - Pros: Version control, readable, template-friendly
     - Cons: File I/O overhead
     - Best for: Large prompt sets, documentation
     - Location: grace/prompts/task_prompts.md

  B) .txt File Approach
     - Pros: Lightweight, simple parsing
     - Cons: Less structured
     - Best for: Simple key-value prompts
     - Location: grace/config/prompts.txt

  C) Runtime Injection (Recommended Hybrid)
     - Pros: Dynamic, flexible, chainable
     - Cons: Requires config system
     - Best for: Task-specific customization
     - Location: grace/config/prompts.json or config.yaml

RECOMMENDATION: Use JSON/YAML config with .md files for documentation
  - grace/config/prompts.yaml (runtime config with references)
  - grace/prompts/ (directory with .md files per task/connector type)

OUTPUT: prompt_framework_design.md with implementation spec
```

### Task 3.2: Prompt Injection Points Mapping
```
EXECUTE: Design injection architecture:
  - Where in code are Claude prompts constructed?
  - How to inject developer-defined prompts?
  - When in execution pipeline should custom prompts apply?
  - How to merge system prompts + developer prompts?

CREATE: Prompt template with variables:
  ```
  Task Prompt Template:
  ===
  [SYSTEM_CONTEXT]
  [DEVELOPER_CUSTOM_PROMPT]
  [TASK_SPECIFIC_INSTRUCTIONS]
  [CONNECTORS_DATA]
  [PREVIOUS_CONTEXT]
  ===
  ```

OUTPUT: prompt_injection_spec.md with:
  - Injection point locations (file:line references)
  - Variable substitution guide
  - Custom prompt syntax/schema
  - Validation rules
```

### Task 3.3: Custom Prompt Schema Definition
```
CREATE: JSON Schema for task-specific prompts
  - Fields: task_name, connector_type, conditions, prompt_text, variables
  - Validation rules: required fields, max length, variable format
  - Examples: payment processor, auth flow, data transformation

OUTPUT: prompt_schema.json with documentation

CREATE: Example custom prompts file:
  - grace/prompts/examples/payment_processor_prompts.md
  - grace/prompts/examples/auth_flow_prompts.md
```

---

## PHASE 4: ENHANCED FEATURE EXTENSION
**Execution Mode: Modify Existing Feature with Backward Compatibility**

### Task 4.1: Current Feature Analysis
```
EXECUTE: Find and read:
  - Current orchestrator feature: "Implement {FLOW} for all connectors in {CONNECTORS_FILE}. Read grace/workflow/1_orchestrator.md and follow it exactly. Integration details: {CONNECTORS_FILE} Branch: {BRANCH}"
  - Location of this feature in codebase
  - How parameters are processed
  - How it integrates with Claude Code

OUTPUT: current_feature_analysis.md
```

### Task 4.2: Enhanced Feature Specification
```
EXTEND Feature to:

OLD:
  "Implement {FLOW} for all connectors in {CONNECTORS_FILE}. Read grace/workflow/1_orchestrator.md and follow it exactly. Integration details: {CONNECTORS_FILE} Branch: {BRANCH}"

NEW (ENHANCED):
  "Implement {FLOW} for all connectors in {CONNECTORS_FILE}. 
   Reference orchestrator: {ORCHESTRATOR_FILE} (supports: 1_orchestrator.md, 2_*, 3_*, 4_*).
   Apply custom prompts from: {CUSTOM_PROMPTS_CONFIG} (path to .md or .yaml).
   Task-specific instructions: {TASK_PROMPT_OVERRIDE}.
   Queue mode: {QUEUE_PROCESSING_MODE} (sequential|parallel|priority).
   Integration details: {CONNECTORS_FILE}.
   Branch: {BRANCH}.
   Output validation schema: {VALIDATION_SCHEMA}.
   Error handling strategy: {ERROR_STRATEGY}."

PARAMETERS TO ADD:
  - {ORCHESTRATOR_FILE}: Flexible orchestrator selection
  - {CUSTOM_PROMPTS_CONFIG}: Developer-provided prompts
  - {TASK_PROMPT_OVERRIDE}: Task-level prompt injection
  - {QUEUE_PROCESSING_MODE}: Queue behavior control
  - {VALIDATION_SCHEMA}: Output validation rules
  - {ERROR_STRATEGY}: Failure handling approach

OUTPUT: enhanced_feature_spec.md
```

---

## PHASE 5: ORCHESTRATOR CONSISTENCY & IMPROVEMENTS
**Execution Mode: File Comparison, Analysis, and Modification**

### Task 5.1: Orchestrator Gap Analysis
```
EXECUTE: Deep comparison of orchestrator files
  
GENERATE: Improvement matrix showing:
  - Features in 1_orchestrator.md missing from 3_* and 4_*
  - Structural inconsistencies
  - Queue integration gaps
  - Prompt injection readiness
  - Claude Code invocation patterns

IDENTIFY REQUIRED IMPROVEMENTS:
  - Missing task definitions
  - Incomplete error handling
  - Queue system references
  - Custom prompt placeholders
  - Validation procedures

OUTPUT: gaps_and_improvements.md with:
  - Detailed comparison table
  - Specific line-by-line improvements needed
  - Migration path from old → new format
```

### Task 5.2: Standardize & Update Orchestrators
```
EXECUTE: Apply improvements to 3_orchestrator.md and 4_orchestrator.md

MODIFICATIONS:
  1. Add queue system integration section
  2. Add custom prompt injection points
  3. Standardize task naming and structure
  4. Add validation checkpoints
  5. Add error handling procedures
  6. Add monitoring/logging instructions
  7. Align with 1_orchestrator.md best practices

VALIDATE: Ensure all files follow same schema/template

OUTPUT: Updated files with changelog.md documenting all changes
```

### Task 5.3: Create Orchestrator Template
```
CREATE: grace/workflow/ORCHESTRATOR_TEMPLATE.md
  - Standard structure for all orchestrators
  - Section for custom prompts
  - Queue configuration section
  - Connector specifications format
  - Validation rules section
  - Error handling procedures

OUTPUT: reusable template for future orchestrators
```

---

## PHASE 6: PAYMENT METHOD CONNECTOR INTEGRATION
**Execution Mode: Complete Implementation with Testing**

### Task 6.1: Analyze Existing Connector Implementation
```
EXECUTE: Choose a reference connector from grace/connectors/
  - Study its structure and implementation
  - Understand tech spec generation
  - Understand queue integration
  - Understand Claude Code invocation
  - Understand validation approach

OUTPUT: connector_reference_analysis.md
```

### Task 6.2: Payment Method Connector Architecture Design
```
CREATE: Detailed specification for payment connector:
  - Tech spec structure for payment processors
  - Supported payment methods (Stripe, PayPal, Square, etc.)
  - Required fields and configurations
  - Validation requirements
  - Queue task definitions
  - Custom prompts for payment logic
  - Error handling for payment-specific issues

OUTPUT: payment_connector_spec.md
```

### Task 6.3: Step-by-Step Integration Guide
```
CREATE: Comprehensive integration guide with commands:

STEP 1: Setup Payment Connector Structure
  mkdir -p grace/connectors/payment_processor
  touch grace/connectors/payment_processor/techspec.md
  touch grace/connectors/payment_processor/config.yaml
  touch grace/connectors/payment_processor/implementation.py

STEP 2: Define Tech Spec Template
  - Payment processor type
  - API credentials location
  - Supported currencies and methods
  - Webhook configuration
  - Transaction logging
  - Refund handling
  - Error codes mapping

STEP 3: Create Custom Prompts
  grace/prompts/payment_processor_prompts.md:
    - Prompt for payment implementation
    - Prompt for webhook handling
    - Prompt for error handling
    - Prompt for testing

STEP 4: Add to Orchestrator
  - Update {ORCHESTRATOR_FILE} with payment_processor task
  - Add queue configuration
  - Add validation rules
  - Add custom prompt references

STEP 5: Queue Integration
  - Register payment_processor tasks in queue config
  - Define priority and retry logic
  - Add completion detection

STEP 6: Implementation
  - Run Claude Code with enhanced feature
  - Generate implementation via custom prompts
  - Validate output against schema
  - Execute and test

STEP 7: Documentation & Handoff
  - Document implementation
  - Create testing procedures
  - Setup monitoring

OUTPUT: step_by_step_integration_guide.md with all commands and examples
```

### Task 6.4: Payment Connector Implementation Script
```
EXECUTE: Generate or create:
  - grace/scripts/setup_payment_connector.sh (bash automation)
  - grace/scripts/test_payment_connector.py (validation)
  - grace/scripts/deploy_payment_connector.sh (deployment)

FEATURES:
  - Automated directory structure creation
  - Tech spec template generation
  - Prompt file creation from examples
  - Orchestrator file update
  - Dependency installation
  - Testing and validation
```

---

## PHASE 7: IMPLEMENTATION & VALIDATION
**Execution Mode: Execute All Changes with Verification**

### Task 7.1: Code Changes Execution
```
EXECUTE ALL CHANGES:
  1. Update orchestrators (3_*, 4_*)
  2. Create prompt framework (config + files)
  3. Enhance main feature with new parameters
  4. Create queue integration layer
  5. Add payment connector foundation
  6. Generate documentation

VALIDATE:
  - All files created successfully
  - No breaking changes to existing features
  - Backward compatibility maintained
  - All references are valid

OUTPUT: implementation_log.md with checksums and file list
```

### Task 7.2: Testing Suite Generation
```
CREATE: grace/tests/test_enhanced_features.py
  - Test custom prompt injection
  - Test queue system integration
  - Test orchestrator flexibility
  - Test payment connector scaffolding
  - Test backward compatibility

EXECUTE: Run all tests and report results

OUTPUT: test_report.md with coverage metrics
```

### Task 7.3: Documentation Generation
```
CREATE: grace/docs/ENHANCED_FEATURES.md
  - Quick start guide
  - Custom prompt usage examples
  - Queue system configuration
  - Payment connector integration tutorial
  - Troubleshooting guide
  - API reference updates

OUTPUT: Complete documentation set
```

---

## EXPECTED OUTPUTS

After execution, you will have:

✅ **Analysis Documents**
  - structure.txt
  - analysis_report.md
  - claude_integration_map.md
  - orchestrator_comparison.md
  - queue_system_analysis.md
  - prompt_framework_design.md

✅ **Enhanced Grace Framework**
  - Updated 3_orchestrator.md and 4_orchestrator.md
  - New prompt management system (config + files)
  - Enhanced feature implementation in codebase
  - Queue integration layer

✅ **Payment Connector Ready-to-Use**
  - grace/connectors/payment_processor/ (scaffolded)
  - payment_connector_spec.md
  - step_by_step_integration_guide.md
  - setup_payment_connector.sh (automation)

✅ **Complete Documentation**
  - ENHANCED_FEATURES.md
  - Implementation logs
  - Test reports
  - Integration guide

---

## EXECUTION INSTRUCTIONS FOR CLAUDE CODE

**Run this prompt as:**
```bash
claude code << 'EOF'
[Insert this entire prompt]

GRACE_REPO_PATH: /path/to/grace/repo
CUSTOM_PROMPTS_LOCATION: grace/prompts/
ORCHESTRATOR_REFERENCE: grace/workflow/1_orchestrator.md
PAYMENT_CONNECTOR_TYPE: stripe  # or paypal, square, etc.
OUTPUT_DIRECTORY: ./grace_enhancements/
EOF
```

**Priority Order:**
1. Phase 1 (Discovery) - Must complete first
2. Phase 2 (Queue Analysis) - Inform Phase 3
3. Phase 3 (Prompt Framework) - Foundation for Phase 4
4. Phase 4-5 (Features & Orchestrators) - Parallel execution
5. Phase 6 (Payment Integration) - After Phase 5
6. Phase 7 (Implementation) - Final execution with validation

---

## KEY CLAUDE CODE OPTIMIZATION KEYWORDS USED

✨ **Execution-Focused:**
  - EXECUTE, ANALYZE, EXTRACT, VALIDATE, MODIFY, CREATE, GENERATE, RUN

✨ **Clarity & Structure:**
  - Sequential, Parallel, Phased, Step-by-step, Deep dive

✨ **Output Definition:**
  - OUTPUT, Expected results, File locations, Format specification

✨ **Action Triggers:**
  - Must complete, Must verify, Must validate, Must test

✨ **Context Enhancement:**
  - Reference files, Comparison matrices, Migration paths

✨ **Automation Keywords:**
  - Automate, Script generation, Batch processing, Template creation
