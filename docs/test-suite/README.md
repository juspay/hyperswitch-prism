<!--
---
title: Test Suite Overview
description: Comprehensive test framework for validating connector functionality across all payment processors with scenario-driven testing and automated reporting
last_updated: 2026-03-12
generated_from: backend/integration-tests/
auto_generated: false
reviewed_by: engineering
reviewed_at: 2026-03-12
approved: true
---
-->

# Test Suite Overview

## Overview

The Connector Service Test Suite is a developer utility designed to validate connector functionality across all 110+ payment connectors. It uses a scenario-driven approach where test behavior is defined in JSON files, making it easy to add new test cases without modifying code.

**Key Benefits:**
- **Scenario-Driven**: Define tests in JSON, not code
- **Dependency Management**: Automatic handling of test dependencies (e.g., capture requires authorize)
- **Comprehensive Reporting**: Auto-generated markdown reports with test matrices
- **CI/CD Ready**: Snapshot testing strategy for continuous validation
- **Multi-Connector**: Test against all connectors or specific ones

## Quick Start

```bash
# Test specific connector
cargo run --bin run_test -- --connector stripe

# Test all connectors
cargo run --bin suite_run_test -- --all

# Run specific suite
cargo run --bin run_test -- --connector stripe --suite authorize

# Run specific scenario
cargo run --bin run_test -- --connector stripe --suite authorize --scenario no3ds_auto_capture_credit_card
```

## Architecture Overview

The test suite is organized into three layers:

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
│  │   Assert     │ │ Credentials  │ │   Report     │        │
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

## Documentation

- [Architecture](./architecture.md) - System design and core components
- [Test Structure](./test-structure.md) - Scenarios, suites, and assertions
- [Global Suites](./global-suites.md) - Reusable test scenarios and dependencies
- [Configuration](./configuration.md) - Test data, credentials, and environment variables
- [Overrides](./overrides.md) - Customizing scenarios per connector
- [Usage](./usage.md) - Running tests and commands
- [CI/CD Integration](./ci-cd.md) - Continuous integration setup
- [Best Practices](./best-practices.md) - Guidelines for writing tests

## Supported Test Suites

| Suite | Service | Description | Dependencies |
|-------|---------|-------------|--------------|
| **authorize** | PaymentService/Authorize | Payment authorization | create_access_token, create_customer |
| **capture** | PaymentService/Capture | Capture authorized payments | authorize |
| **void** | PaymentService/Void | Void authorized payments | authorize |
| **refund** | PaymentService/Refund | Refund captured payments | capture |
| **get** | PaymentService/Get | Retrieve payment status | authorize |
| **create_access_token** | MerchantAuthenticationService | Create authentication tokens | none |
| **create_customer** | CustomerService/Create | Create customer profiles | create_access_token |
| **setup_recurring** | PaymentService/SetupRecurring | Setup recurring mandates | authorize |
| **recurring_charge** | RecurringPaymentService/Charge | Charge using mandate | setup_recurring |
| **refund_sync** | RefundService/Get | Sync refund status | refund |

## Environment Variables

| Variable | Required | Description |
|----------|----------|-------------|
| `CONNECTOR_AUTH_FILE_PATH` | Yes | Path to connector credentials JSON file |
| `UCS_CREDS_PATH` | Alternative | Alternative path for credentials |
| `UCS_SCENARIO_ROOT` | No | Override scenario files location |
| `UCS_RUN_TEST_REPORT_PATH` | No | Custom report output path |
| `UCS_ALL_CONNECTORS` | No | Comma-separated list of connectors to test |

## Next Steps

1. Read the [Architecture](./architecture.md) guide to understand the system design
2. Review [Test Structure](./test-structure.md) to learn how scenarios work
3. Check [Configuration](./configuration.md) for setting up credentials
4. See [Usage](./usage.md) for running your first test
