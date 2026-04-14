<!--
---
title: Test Suite Architecture
description: System design and core components of the Connector Service test framework
last_updated: 2026-03-12
generated_from: backend/integration-tests/src/harness/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Architecture

## Overview

The Connector Service Test Suite uses a **scenario-driven architecture** where test behavior is defined in JSON files rather than code. This enables rapid test development, easy maintenance, and consistent testing across all 110+ payment connectors.

## Design Principles

1. **Data-Driven**: Test logic in JSON, execution in Rust
2. **Reusable**: Global suites work across all connectors
3. **Composable**: Dependencies chain together naturally
4. **Extensible**: Easy to add new scenarios and connectors
5. **Observable**: Comprehensive reporting and logging

## System Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                     Test Definitions                        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │ scenario.json│ │ suite_spec   │ │ connector    │        │
│  │ (test cases) │ │ .json        │ │ _spec.json   │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Test Harness                             │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │   Loader     │ │  Executor    │ │   Server     │        │
│  │(scenario_    │ │(gRPC calls)  │ │  (UCS spawn) │        │
│  │ loader.rs)   │ │              │ │              │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │   Assert     │ │ Credentials  │ │   Report     │        │
│  │(scenario_    │ │   (auth      │ │  Generator   │        │
│  │ _assert.rs)  │ │   loading)   │ │              │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
└──────────────────────────┬──────────────────────────────────┘
                           │
                           ▼
┌─────────────────────────────────────────────────────────────┐
│                    Output Artifacts                         │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐        │
│  │ report.json  │ │test_report   │ │ Connector    │        │
│  │ (raw data)   │ │   .md        │ │   reports    │        │
│  └──────────────┘ └──────────────┘ └──────────────┘        │
└─────────────────────────────────────────────────────────────┘
```

## Core Components

### Scenario Loader

**File**: `harness/scenario_loader.rs`

Loads and validates scenario JSON files from disk.

**Responsibilities**:
- Discover available suites and scenarios
- Load `scenario.json` and `suite_spec.json` files
- Resolve scenario paths from environment variables
- Validate scenario structure

**Key Functions**:
```rust
pub fn load_scenario(suite: &str, scenario: &str) -> Result<ScenarioDef, ScenarioError>
pub fn load_suite_spec(suite: &str) -> Result<SuiteSpec, ScenarioError>
pub fn scenario_root() -> PathBuf
```

### Executor

**File**: `harness/executor.rs`

Spawns the Connector Service server and executes gRPC calls.

**Responsibilities**:
- Start in-memory UCS server for testing
- Create gRPC clients for each service
- Add authentication metadata to requests
- Manage request/response lifecycle

**Key Struct**:
```rust
pub struct ConnectorExecutor {
    server: UcsServer,
    auth: ConnectorAuth,
    connector: String,
    merchant_id: String,
    tenant_id: String,
}
```

### Assertion Engine

**File**: `harness/scenario_assert.rs`

Validates gRPC responses against assertion rules.

**Responsibilities**:
- Evaluate `must_exist`, `must_not_exist` rules
- Compare values with `equals` and `one_of`
- Check string containment with `contains`
- Echo request fields with `echo`

**Assertion Types**:
```rust
pub enum FieldAssert {
    MustExist { must_exist: bool },
    MustNotExist { must_not_exist: bool },
    Equals { equals: Value },
    OneOf { one_of: Vec<Value> },
    Contains { contains: String },
    Echo { echo: String },
}
```

### Report Generator

**File**: `harness/report.rs`

Generates test reports in JSON and Markdown formats.

**Responsibilities**:
- Record test execution results
- Calculate pass/fail statistics
- Generate markdown tables and matrices
- Sanitize sensitive data in reports

**Output Files**:
- `report.json` - Machine-readable test results
- `test_report.md` - Human-readable markdown summary

### Credentials Manager

**File**: `harness/credentials.rs`

Loads and manages connector authentication credentials.

**Responsibilities**:
- Load credentials from JSON file
- Support multiple authentication types
- Handle connector-specific labels
- Cache credentials for reuse

**Authentication Types**:
```rust
pub enum ConnectorAuth {
    HeaderKey { api_key: String },
    BodyKey { api_key: String, key1: String },
    SignatureKey { api_key: String, key1: String, api_secret: String },
}
```

### Auto-Generator

**File**: `harness/auto_gen.rs`

Generates dynamic test data for `auto_generate` placeholders.

**Responsibilities**:
- Generate unique transaction IDs
- Create realistic customer data
- Generate test email addresses
- Create phone numbers and addresses

**Generated Types**:
- Transaction IDs: `mti_{uuid}`, `mri_{uuid}`, `cti_{uuid}`
- Emails: `alex.1234@example.com`
- Phone numbers: `+15551234567`
- Names: `Emma Johnson`

## Execution Flow

```
1. Load Suite Spec
   └─> Read suite_spec.json for dependencies

2. Execute Dependencies
   └─> Run dependent suites first (authorize before capture)
   └─> Capture context from responses

3. Resolve auto_generate
   └─> Generate unique IDs and dynamic data
   └─> Skip context-deferred fields

4. Send Request
   └─> Create gRPC request with metadata
   └─> Add connector authentication
   └─> Execute RPC

5. Assert Response
   └─> Validate against assert rules
   └─> Record pass/fail status

6. Generate Report
   └─> Append to report.json
   └─> Regenerate test_report.md
```

## Directory Structure

```
backend/integration-tests/
├── src/
│   ├── bin/
│   │   ├── run_test.rs          # Single test runner
│   │   └── suite_run_test.rs    # Suite runner binary
│   ├── harness/
│   │   ├── mod.rs
│   │   ├── auto_gen.rs          # Dynamic data generation
│   │   ├── credentials.rs       # Auth loading
│   │   ├── executor.rs          # gRPC execution
│   │   ├── metadata.rs          # Request metadata
│   │   ├── report.rs            # Report generation
│   │   ├── scenario_api.rs      # Public API
│   │   ├── scenario_assert.rs   # Assertion engine
│   │   ├── scenario_loader.rs   # File loading
│   │   └── scenario_types.rs    # Data structures
│   ├── global_suites/           # Reusable test scenarios
│   │   ├── authorize_suite/
│   │   ├── capture_suite/
│   │   └── ...
│   └── connector_specs/         # Per-connector config
│       ├── stripe.json
│       └── ...
├── Cargo.toml
└── README.md
```

## Key Design Decisions

### Why JSON for Test Definitions?

- **Readable**: Easy to understand and modify
- **Version Control**: Clean diffs in git
- **Non-Programmer Friendly**: QA teams can write tests
- **Tooling**: Standard format with editor support

### Why Global Suites?

- **DRY Principle**: Write once, test all connectors
- **Consistency**: Same validation across connectors
- **Maintenance**: Update one place, affect all
- **Coverage**: Ensures all connectors support core features

### Why Dependency Pipeline?

- **Realistic Flows**: Tests mirror production usage
- **Data Sharing**: Transaction IDs flow between steps
- **Reduced Boilerplate**: No manual setup/teardown
- **Composable**: Build complex flows from simple pieces

## Next Steps

- [Test Structure](./test-structure.md) - Learn about scenarios and assertions
- [Global Suites](./global-suites.md) - Understand reusable test scenarios
- [Configuration](./configuration.md) - Set up credentials and environment
