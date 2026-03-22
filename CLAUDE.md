# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is an open-source payments connector service built in Rust with gRPC. It provides a unified interface for integrating with multiple payment processors (Stripe, Adyen, Razorpay, etc.) through a processor-agnostic API. The service acts as an abstraction layer that allows merchants to switch between payment processors without changing their business logic.

## Development Commands

### Building and Running
```bash
# Build the project
cargo build

# Run the development server
cargo run

# Run with Kafka features enabled
cargo run --features kafka

# Run specific package
cargo run -p grpc-server
```

### Code Quality and Testing
```bash
# Format code (uses nightly rustfmt)
cargo +nightly fmt --all
# OR use make
make fmt

# Run clippy for linting
cargo clippy
# OR comprehensive check with cargo-hack
make clippy

# Run all tests with feature combinations
make test
# OR manually
cargo hack test --each-feature

# Run specific test
cargo test <test_name>

# Run tests for specific package
cargo test -p grpc-server

# Run integration tests
cargo test --test integration_tests

# Run all quality checks (fmt, check, clippy, test)
make all

# CI mode (treats warnings as errors)
make ci
```

### Protobuf Operations
```bash
# Format proto files
make proto-format

# Generate code from proto files
make proto-generate

# Validate proto files
make proto-build

# Lint proto files
make proto-lint

# Clean generated proto files
make proto-clean
```

### Running Tests by Connector
```bash
cargo test adyen_integration_test
cargo test razorpay_integration_test
cargo test fiserv_integration_test
cargo test elavon_integration_test
```

## Architecture Overview

### Core Components

**grpc-server** (`backend/grpc-server/`)
- Main gRPC service implementation
- Handles incoming requests and routing to appropriate connector integrations
- Contains server implementations for Payment, Refund, and Dispute services
- Manages configuration, logging, metrics, and metadata handling

**connector-integration** (`backend/connector-integration/`)
- Payment processor-specific implementations
- Each connector (Adyen, Razorpay, etc.) implements the `ConnectorIntegration` trait
- Handles transformation between generic payment data and processor-specific formats
- Contains webhook processing logic via `IncomingWebhook` trait

**domain-types** (`backend/domain_types/`)
- Common data structures shared between grpc-server and connector-integration
- Defines router data, payment flow data, and type conversions
- Contains `ForeignTryFrom` trait implementations for type safety

**grpc-api-types** (`backend/grpc-api-types/`)
- Auto-generated gRPC types from `.proto` files
- Defines service interfaces and data structures for client-server communication

### Key Architectural Patterns

**Connector Integration Trait System**
```rust
trait ConnectorIntegration<Flow, ResourceCommonData, Req, Resp> {
    fn get_headers();
    fn get_content_type(); 
    fn get_http_method();
    fn get_url();
    fn get_request_body();
    fn build_request();
    fn handle_response();
    fn get_error_response();
}
```

**Webhook Processing**
```rust
trait IncomingWebhook {
    fn verify_webhook_source();
    fn get_event_type();
    fn process_payment_webhook();
    fn process_refund_webhook();
}
```

**Type-Safe Conversions**
- Uses `ForeignTryFrom` trait for safe type conversions between gRPC types and domain types
- Ensures compile-time type safety across service boundaries

### Data Flow

1. **Incoming Request**: gRPC client sends request to grpc-server
2. **Metadata Extraction**: Server extracts authentication, tenant info, and other metadata
3. **Type Conversion**: gRPC types converted to domain types via `ForeignTryFrom`
4. **Connector Selection**: Request routed to appropriate connector based on metadata
5. **Processor Integration**: Connector transforms request to processor-specific format
6. **External API Call**: HTTP request sent to payment processor
7. **Response Processing**: Processor response transformed back to common format
8. **Response Return**: Common response converted to gRPC types and returned

### Security and Masking

The codebase uses `hyperswitch-masking` library for PCI compliance:
- Sensitive data (API keys, card numbers) wrapped in `Secret<T>` types
- Automatic masking in logs and serialization
- `ExposeInterface` trait for controlled access to sensitive data

### Configuration Management

Configuration is environment-based with TOML files:
- `config/development.toml` - Development environment
- `config/production.toml` - Production environment  
- `config/sandbox.toml` - Sandbox environment

Environment determined by `CS__COMMON__ENVIRONMENT` variable.

### Key Modules

**Metadata Handling** (`grpc-server/src/utils.rs`)
- `MetadataPayload` struct contains all extracted gRPC metadata
- Functions for extracting tenant ID, merchant ID, auth details, lineage IDs
- `grpc_logging_wrapper` for request/response logging with structured metadata

**Error Handling** (`grpc-server/src/error.rs`)
- Custom error types with gRPC status code mapping
- `IntoGrpcStatus` trait for converting domain errors to gRPC errors

**Metrics and Observability** 
- Prometheus metrics endpoint on separate port
- Structured logging with tracing
- Optional Kafka logging for production environments

## Development Guidelines

### Workspace Structure
This is a Cargo workspace with multiple crates. Key dependencies flow:
- `grpc-server` depends on all other backend crates
- `connector-integration` depends on `domain_types` and utility crates
- `grpc-api-types` is generated from protobuf definitions

### Lint Configuration
The workspace enforces strict linting rules:
- `unsafe_code = "forbid"` - No unsafe Rust allowed
- Comprehensive clippy rules including panic detection
- Use `cargo +nightly fmt` for formatting (nightly required)

### Testing Strategy
- Unit tests per crate
- Integration tests in `grpc-server/tests/`
- Connector-specific test files for each payment processor
- Use `cargo hack test --each-feature` to test feature combinations

### Adding New Connectors
1. Create new module in `connector-integration/src/connectors/`
2. Implement `ConnectorIntegration` trait for each flow (Authorize, Capture, etc.)
3. Implement `IncomingWebhook` trait for webhook processing
4. Add connector configuration to config files
5. Add integration tests in `grpc-server/tests/`

### Server Features
- **Default**: Core gRPC functionality
- **kafka**: Enables Kafka-based logging via `tracing-kafka` crate

The server supports graceful shutdown via Unix signals and provides health check endpoints for Kubernetes deployments.

## Project-Specific Instructions

### Metadata Security Implementation
When working with gRPC metadata, always follow these security patterns:
- Use `MetadataPayload` struct instead of passing raw `tonic::metadata::MetadataMap`
- Extract all metadata fields once in `get_metadata_payload()` function
- Vault credentials and auth keys must be wrapped in `Secret<String>` types
- Never pass raw metadata to business logic - use structured `MetadataPayload` fields
- Authentication data is already extracted in `metadata_payload.connector_auth_type`

### Code Quality Standards
- Use production-grade variable names, avoid task-specific naming
- Prefer explicit imports over inline imports
- Always prefer editing existing files over creating new ones
- Never create documentation files unless explicitly requested
- Do what is asked, nothing more, nothing less

### Rust Idiomatic Code Practices
- Write idiomatic, clean, and maintainable Rust code following best practices
- Prefer shorter, cleaner code when it improves readability without sacrificing clarity
- Use functional programming patterns where appropriate (iterators, closures, combinators)
- Leverage Rust's type system for compile-time safety and zero-cost abstractions
- Use pattern matching (`match`, `if let`) instead of verbose conditional chains
- Prefer `?` operator for error propagation over explicit error handling
- Use method chaining and iterator combinators for data transformations
- Group related methods and functionality logically within modules
- Avoid unnecessary complexity - favor simple, direct solutions
- Use `derive` macros for common traits when possible
- Prefer `impl Trait` for return types when appropriate
- Use proper error types with `thiserror` for domain-specific errors

### Current Refactoring Context
The codebase is undergoing metadata security refactoring to eliminate raw metadata passing:
- `MetadataPayload` contains all structured, masked metadata fields
- Remove redundant `auth_from_metadata()` calls in business logic
- Domain constructors should use `MetadataPayload` fields instead of raw metadata
- The `grpc_logging_wrapper` function extracts metadata early to prevent security leaks