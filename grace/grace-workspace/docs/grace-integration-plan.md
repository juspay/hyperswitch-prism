# 10XGRACE + Grace Workflow Integration Plan

## Overview

This document outlines how to adapt 10XGRACE's frontend automation pipeline to use Grace's hierarchical agent-based workflow architecture for implementing payment connector features.

### Current State

**10XGRACE (Frontend Pipeline)**
- Linear 17-checkpoint pipeline for hyperswitch-control-center (ReScript/React)
- Uses OpenCode CLI for implementation steps
- Supports concurrent file processing (4 workers)
- Testing via Cypress/Playwright with visual diff

**Grace (Backend Workflow)**
- Hierarchical agent architecture for payment connectors (Rust)
- Markdown workflow files defining agent behavior
- Strict sequential processing (one connector at a time)
- Testing via cargo build + grpcurl
- Git workflow: commit → cherry-pick → PR

### Target State

A hybrid system that:
1. Uses Grace's hierarchical agent orchestration
2. Maintains 10XGRACE's 17-checkpoint pipeline per feature
3. Processes multiple features sequentially (never parallel)
4. Integrates with existing `creds.json` for credentials
5. Supports both frontend and backend automation

---

## Architecture

### Hierarchical Agent Structure

```
┌─────────────────────────────────────────────────────────────────┐
│                    ORCHESTRATOR AGENT                           │
│              (1_orchestrator.md - Sequential Coordinator)       │
│                                                                 │
│  Inputs: features.json, creds.json, branch name                 │
│  Behavior:                                                      │
│    - Load feature list from features.json                       │
│    - Check credentials in creds.json                            │
│    - Process ONE feature at a time (strictly sequential)        │
│    - Stay on working branch throughout                          │
│    - Spawn Feature Agent per feature                            │
│    - Collect results and report summary                         │
└───────────────────────────┬─────────────────────────────────────┘
                            │ Task Tool (one at a time)
                            ▼
┌─────────────────────────────────────────────────────────────────┐
│                    FEATURE AGENT                                │
│           (2_feature_agent.md - Per-Feature Handler)            │
│                                                                 │
│  Manages all 17 checkpoints for a single feature:               │
│    1. Task Definition                                           │
│    2. Product Alignment                                         │
│    3. Feature Research                                          │
│    4. Design Gate                                               │
│    5. L2 Specification (spawn L2 Agent)                         │
│    6. L2 Review                                                 │
│    7. L3 Specification (spawn L3 Agent)                         │
│    8. L3 Review                                                 │
│    9. L4 Specification (spawn L4 Agent)                         │
│    10. L4 Review                                                │
│    11. Implementation (spawn Implementation Agent)              │
│    12. Compiler                                                 │
│    13. Design Match                                             │
│    14. Cypress Tests                                            │
│    15. Playwright Tests                                         │
│    16. PR Review                                                │
│    17. Regression                                               │
│                                                                 │
│  Each "spawn" delegates to a subagent via Task tool             │
└─────────────────────────────────────────────────────────────────┘
```

### Subagent Hierarchy

| Level | Agent | File | Responsibility |
|-------|-------|------|----------------|
| 1 | Orchestrator | `1_orchestrator.md` | Coordinate multiple features sequentially |
| 2 | Feature Agent | `2_feature_agent.md` | Manage 17 checkpoints for one feature |
| 3 | L2 Agent | `2.1_l2_agent.md` | Generate L2 specification |
| 3 | L3 Agent | `2.2_l3_agent.md` | Generate L3 specification |
| 3 | L4 Agent | `2.3_l4_agent.md` | Generate L4 specification |
| 3 | Implementation Agent | `2.4_implementation_agent.md` | Generate code for all files |
| 3 | Testing Agent | `2.5_testing_agent.md` | Run compiler + E2E tests |
| 3 | PR Agent | `2.6_pr_agent.md` | Git commit + PR creation |

---

## File Structure Changes

### New Workflow Directory

```
10xgrace/
├── workflow/                              # NEW: Markdown workflow files
│   ├── 1_orchestrator.md                  # Top-level coordinator
│   ├── 2_feature_agent.md                 # Per-feature handler
│   ├── 2.1_l2_agent.md                    # L2 specification generation
│   ├── 2.2_l3_agent.md                    # L3 specification generation
│   ├── 2.3_l4_agent.md                    # L4 specification generation
│   ├── 2.4_implementation_agent.md        # Code generation
│   ├── 2.5_testing_agent.md               # Compiler + E2E tests
│   └── 2.6_pr_agent.md                    # Git operations + PR
```

### New Configuration Files

```
10xgrace/
├── config.yml                             # EXTENDED: Add grace section
├── features.json                          # NEW: Feature list (like connectors.json)
├── creds.json -> /Users/jeeva.ramachandran/Downloads/creds.json  # SYMLINK
```

### Modified Source Files

```
10xgrace/packages/core/src/
├── agents/                                # NEW: Agent runners
│   ├── orchestrator.ts
│   ├── feature-agent.ts
│   └── subagent-spawner.ts
├── git/                                   # NEW: Git workflow
│   └── workflow.ts
├── checkpoints/                           # MODIFIED: Refactor to use subagents
│   ├── l2-gen.ts          -> delegates to 2.1_l2_agent.md
│   ├── l3-gen.ts          -> delegates to 2.2_l3_agent.md
│   ├── l4-gen.ts          -> delegates to 2.3_l4_agent.md
│   ├── implementation.ts  -> delegates to 2.4_implementation_agent.md
│   └── pr-review.ts       -> delegates to 2.6_pr_agent.md
```

---

## Configuration Schema

### Extended config.yml

```yaml
# Existing 10xgrace configuration
projectRoot: ../hyperswitch-control-center
devServerUrl: http://localhost:9000
designMatchThreshold: 0.90
maxRetries: 3
dashboardPort: 3141
wsPort: 3142

# NEW: Grace workflow integration
grace:
  enabled: true
  workflowDir: ./workflow
  featuresFile: ./features.json
  credsFile: /Users/jeeva.ramachandran/Downloads/creds.json
  sequentialProcessing: true        # HARD REQUIREMENT: never parallel
  devBranch: feat/multi-feature       # Shared branch for all features
  prBranchPrefix: pr/feature          # Prefix for PR branches

# NEW: Git workflow configuration
git:
  commitMessageTemplate: "feat({feature}): {checkpoint}"
  autoPush: false                     # Require manual approval for push
  scrubCredentials: true              # Remove creds from commits

# Existing opencode/llm config (unchanged)
opencode:
  model: "litellm/kimi-latest"
  attachUrl: "http://127.0.0.1:4096"
  timeoutMs: 900000
  implementationConcurrency: 4        # Note: per-file, not per-feature

llm:
  baseUrl: "https://grid.ai.juspay.net/v1/chat/completions"
  apiKey: ""
  model: "kimi-latest"
  protocol: openai

# Existing checkpoints config (unchanged)
checkpoints:
  compiler:
    command: npm
    args: ["run", "re:start"]
  cypress:
    command: npm
    args: ["run", "cy:run"]
  playwright:
    command: npm
    args: ["run", "pw:test"]
  regression:
    enabled: false
```

### features.json Format

```json
[
  {
    "name": "ApplePayButton",
    "title": "Add Apple Pay button to checkout page",
    "description": "Implement Apple Pay button component with proper styling and payment flow integration. Support both light and dark modes.",
    "acceptanceCriteria": [
      "Apple Pay button displays correctly in checkout",
      "Button follows Apple's Human Interface Guidelines",
      "Payment flow works end-to-end with test cards",
      "Component is responsive on mobile and desktop"
    ],
    "figmaUrl": "https://figma.com/file/xxx/apple-pay",
    "targetFiles": ["src/components/ApplePayButton.res"],
    "requiresCredentials": ["applePayMerchantId"],
    "paymentConnectors": ["stripe", "adyen"]
  },
  {
    "name": "PaymentRetryUI",
    "title": "Add retry UI for failed payments",
    "description": "Create a user-friendly retry interface when payments fail...",
    "acceptanceCriteria": [
      "Retry UI appears on payment failure",
      "User can modify payment details",
      "Clear error messaging displayed"
    ]
  }
]
```

---

## Workflow File Specifications

### 1. Orchestrator Agent (1_orchestrator.md)

**Purpose**: Coordinate multiple features sequentially

**Inputs**:
- `FEATURES_FILE`: Path to features.json
- `BRANCH`: Git branch name for all work
- `CREDS_FILE`: Path to creds.json

**Behavior**:

```markdown
# Orchestrator Agent

## STEP 1: PRE-FLIGHT

1. Load features.json
2. Load creds.json
3. Create working branch: git checkout -b {BRANCH}
4. Validate each feature has required credentials
5. Build FEATURE_LIST (skip features without credentials)

## STEP 2: PROCESS FEATURES (ONE AT A TIME)

FOR each feature in FEATURE_LIST:
  - Spawn Feature Agent via Task tool
  - Wait for completion
  - Collect result (SUCCESS/FAILED/SKIPPED)
  - Proceed to next feature

NEVER spawn multiple Feature Agents in parallel.

## STEP 3: REPORT

Print summary:
- Total features processed
- Successful / Failed / Skipped counts
- Per-feature status with reasons
```

### 2. Feature Agent (2_feature_agent.md)

**Purpose**: Execute all 17 checkpoints for one feature

**Inputs**:
- `FEATURE_NAME`: Feature identifier
- `FEATURE_DATA`: Full feature object from features.json
- `BRANCH`: Working branch name

**Checkpoint Flow**:

```markdown
# Feature Agent

## Phase 1: Setup (checkpoints 1-4)
- Run locally (no subagent)
- Task, Product Alignment, Feature Research, Design Gate

## Phase 2: Specification (checkpoints 5-10)
- Spawn L2 Agent → wait → review
- Spawn L3 Agent → wait → review  
- Spawn L4 Agent → wait → review

## Phase 3: Implementation (checkpoint 11)
- Spawn Implementation Agent
- Wait for all files to be generated

## Phase 4: Validation (checkpoints 12-16)
- Compiler (local)
- Design Match (local, if needed)
- Cypress (local)
- Playwright (local)
- PR Review (spawn PR Agent)

## Phase 5: Cleanup (checkpoint 17)
- Regression tests (local)

## Hard Rules:
- Sequential checkpoint execution
- Retry from specified checkpoint on failure
- Max 3 retries per checkpoint (configurable)
- Never commit code that hasn't passed compiler + at least one E2E test
```

### 3. L2 Agent (2.1_l2_agent.md)

**Purpose**: Generate L2 specification (scope, constraints, complexity)

**Inputs**:
- Feature title, description, acceptance criteria
- Existing codebase structure (via tools)

**Output**: L2Spec object

```markdown
# L2 Agent

Read the existing codebase to understand:
- Current component structure
- Similar features already implemented
- Technical constraints

Generate L2 specification:
- Summary
- Scope (in/out)
- Technical constraints
- Estimated complexity (low/medium/high)

Return structured JSON matching L2Spec type.
```

### 4. L3 Agent (2.2_l3_agent.md)

**Purpose**: Generate L3 specification (architecture, tasks, dependencies)

**Inputs**:
- L2 specification
- Feature requirements
- Codebase analysis

**Output**: L3Spec object with tasks and dependencies

### 5. L4 Agent (2.3_l4_agent.md)

**Purpose**: Generate L4 specification (file-level changes)

**Inputs**:
- L3 specification
- Task breakdown

**Output**: L4Spec with subtasks per file

### 6. Implementation Agent (2.4_implementation_agent.md)

**Purpose**: Generate code for all files in L4 spec

**Inputs**:
- L4 specification
- Existing file contents (if modifying)

**Output**: ImplementationResult with all file contents

```markdown
# Implementation Agent

FOR each subtask in L4 spec:
  1. Read current file contents (if exists)
  2. Generate new contents using tools
  3. Match project conventions (ReScript/React)
  4. Return complete file contents

Return all files as ImplementationResult.
```

### 7. Testing Agent (2.5_testing_agent.md)

**Purpose**: Run compiler and E2E tests

**Inputs**:
- Project root
- Modified files

**Output**: TestReport

```markdown
# Testing Agent

1. Run ReScript compiler
   - Must pass before E2E tests
   - Capture all errors

2. Start dev server

3. Run Cypress tests
   - Capture screenshots on failure

4. Run Playwright tests
   - Capture screenshots on failure

5. Compare visual diffs (if design gate enabled)

Return TestReport with:
- totalTests, passed, failed
- Failure details with screenshots
- Visual diff results
```

### 8. PR Agent (2.6_pr_agent.md)

**Purpose**: Git operations and PR creation

**Inputs**:
- Feature name
- Modified files
- Working branch
- Test results

**Output**: PR URL

```markdown
# PR Agent

1. Stage feature-specific files ONLY
   - git add <specific files>
   - NEVER git add -A

2. Commit to working branch
   - Message: "feat({feature}): implementation"

3. Create clean PR branch
   - git checkout -b {pr-branch}

4. Cherry-pick commit

5. Scrub credentials from code
   - Remove API keys, secrets
   - Replace with placeholders

6. Push PR branch

7. Create GitHub PR
   - Include test results
   - Link to feature spec

Return PR URL.
```

---

## Implementation Phases

### Phase 1: Infrastructure (Week 1)

**Tasks**:
1. Create `workflow/` directory with all .md files
2. Extend `config.ts` with grace/git sections
3. Create `features.json` schema and loader
4. Create credential checker
5. Implement git workflow utilities

**Deliverables**:
- [ ] 8 workflow markdown files
- [ ] Extended config types
- [ ] Credential validation
- [ ] Git workflow utilities

### Phase 2: Subagent Spawner (Week 1-2)

**Tasks**:
1. Create `subagent-spawner.ts` with Task tool integration
2. Define subagent result types
3. Implement retry logic
4. Add error handling

**Deliverables**:
- [ ] Task tool wrapper
- [ ] Result parsers
- [ ] Retry mechanism

### Phase 3: Orchestrator Implementation (Week 2)

**Tasks**:
1. Implement Orchestrator Agent
2. Add sequential processing enforcement
3. Integrate credential checking
4. Build result aggregation

**Deliverables**:
- [ ] Orchestrator can load and validate features
- [ ] Sequential processing works
- [ ] Summary report generated

### Phase 4: Feature Agent Migration (Week 3)

**Tasks**:
1. Migrate L2/L3/L4 generation to subagents
2. Migrate implementation to subagent
3. Migrate PR creation to subagent
4. Keep local checkpoints (compiler, tests)

**Deliverables**:
- [ ] Feature Agent coordinates all 17 checkpoints
- [ ] Subagents spawn correctly
- [ ] Results flow back properly

### Phase 5: Testing & Integration (Week 4)

**Tasks**:
1. End-to-end testing with sample features
2. Test credential skipping
3. Test git workflow
4. Dashboard updates for multi-feature view

**Deliverables**:
- [ ] Sample features process correctly
- [ ] Git workflow verified
- [ ] Dashboard shows multi-feature progress

---

## Key Rules & Guardrails

### Sequential Processing (HARD REQUIREMENT)

```typescript
// Enforced at orchestrator level
class Orchestrator {
  private currentFeature: string | null = null;
  
  async processFeature(feature: Feature): Promise<Result> {
    if (this.currentFeature) {
      throw new Error(
        'PARALLEL_EXECUTION_BLOCKED: ' +
        'Cannot process multiple features simultaneously. ' +
        'This prevents git branch corruption.'
      );
    }
    
    this.currentFeature = feature.name;
    try {
      return await this.spawnFeatureAgent(feature);
    } finally {
      this.currentFeature = null;
    }
  }
}
```

### Credential Validation

```typescript
function validateCredentials(
  feature: Feature,
  creds: Credentials
): ValidationResult {
  const missing: string[] = [];
  
  for (const cred of feature.requiresCredentials || []) {
    if (!hasCredential(creds, cred)) {
      missing.push(cred);
    }
  }
  
  if (missing.length > 0) {
    return {
      valid: false,
      action: 'SKIP',
      reason: `Missing credentials: ${missing.join(', ')}`
    };
  }
  
  return { valid: true };
}
```

### Scoped Git Operations

```typescript
// CORRECT
await gitAdd([
  'src/components/ApplePayButton.res',
  'src/components/ApplePayButton.resi'
]);

// WRONG - NEVER DO THIS
await gitAdd('.');  // DON'T
await gitAdd('-A'); // DON'T
```

### Commit Requirements

```typescript
async function commitFeature(
  feature: Feature,
  checkpoint: Checkpoint
): Promise<void> {
  // 1. Must have passed compiler
  if (!checkpoint.results.compiler?.passed) {
    throw new Error('Cannot commit: compiler failed');
  }
  
  // 2. Must have passed at least one E2E test
  const e2ePassed = 
    checkpoint.results.cypress?.passed > 0 ||
    checkpoint.results.playwright?.passed > 0;
  
  if (!e2ePassed) {
    throw new Error('Cannot commit: no E2E tests passed');
  }
  
  // 3. Only stage feature files
  await stageFeatureFiles(feature.name);
  
  // 4. Commit
  await commit(`feat(${feature.name}): ${checkpoint.id}`);
}
```

---

## Testing Strategy

### Unit Tests

```typescript
// Test credential validator
describe('credentialValidator', () => {
  it('should skip feature without credentials', () => {
    const feature = {
      name: 'Test',
      requiresCredentials: ['apiKey']
    };
    const creds = {};
    
    const result = validateCredentials(feature, creds);
    
    expect(result.valid).toBe(false);
    expect(result.action).toBe('SKIP');
  });
});

// Test sequential enforcement
describe('orchestrator', () => {
  it('should prevent parallel feature processing', async () => {
    const orch = new Orchestrator();
    
    // Start first feature
    const p1 = orch.processFeature(feature1);
    
    // Second feature should throw
    await expect(orch.processFeature(feature2))
      .rejects.toThrow('PARALLEL_EXECUTION_BLOCKED');
    
    await p1;
  });
});
```

### Integration Tests

```typescript
// Test full feature pipeline
describe('feature pipeline', () => {
  it('should process feature end-to-end', async () => {
    const feature = loadFixture('apple-pay');
    
    const result = await runFeaturePipeline(feature);
    
    expect(result.status).toBe('SUCCESS');
    expect(result.checkpoints).toHaveLength(17);
    expect(result.prUrl).toMatch(/github.com\/.*\/pull\/\d+/);
  }, 600000); // 10 minute timeout
});
```

---

## Migration Guide

### For Existing 10XGRACE Users

1. **Update config.yml**: Add `grace:` section
2. **Create features.json**: Migrate from single task
3. **Link creds.json**: Point to existing credentials
4. **Enable gradually**: Set `grace.enabled: false` initially, then `true` after testing

### Backwards Compatibility

```typescript
// Support both modes
if (config.grace?.enabled) {
  // Use hierarchical agent workflow
  await runGraceWorkflow(features);
} else {
  // Use legacy linear pipeline
  await runLegacyPipeline(task);
}
```

---

## Success Metrics

| Metric | Target |
|--------|--------|
| Features processed without parallel execution | 100% |
| Credentials validated before processing | 100% |
| Git commits scoped to feature files only | 100% |
| Features passing compiler + E2E before PR | 100% |
| Average processing time per feature | < 30 min |
| Dashboard multi-feature visibility | Yes |

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Git branch corruption | Strict sequential processing + scoped commits |
| Credential leaks | Automated scrubbing in PR Agent |
| Infinite retries | Max retry limit + exponential backoff |
| Lost work | SQLite state + resume capability |
| Conflicting features | Sequential processing prevents conflicts |

---

## Timeline Summary

- **Week 1**: Infrastructure + workflow files
- **Week 2**: Subagent spawner + orchestrator
- **Week 3**: Feature agent migration
- **Week 4**: Testing + polish

**Total: 4 weeks to full integration**

---

## Next Steps

1. Review this plan with stakeholders
2. Approve approach and timeline
3. Begin Phase 1 implementation
4. Set up feature flag for gradual rollout
