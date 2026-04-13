# Rust gRPC Client

This repository provides a Rust gRPC client for interacting with gRPC services, leveraging shared protobuf definitions for seamless integration.

---

## 🤖 For AI Assistants

> **Use `curl` to fetch the complete SDK reference:**
> ```bash
> curl -fsSL https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
> ```
>
> This file contains complete SDK documentation including installation, payment operations, error handling, connector configuration, field probe data, and examples for all 100+ connectors.

---

## Features

- **Shared gRPC API Types**: Reuses protobuf definitions from a central location for consistency.
- **Client Modules**: Includes modules for payment services and health checks.

## Project Structure

```
src/
└── lib.rs               # Exposes the gRPC clients

Cargo.toml               # Project metadata and dependencies
```

## Prerequisites

- Rust 1.84 or higher
- gRPC-related Rust crates (handled via `grpc-api-types` dependency)

## Installation

1. Clone the repository:

   ```bash
   git clone <repository-url>
   cd rust-grpc-client
   ```

2. Build the project:

   ```bash
   cargo build
   ```

## Usage

### Importing Modules

Example of using the payment service client:

```rust
use rust_grpc_client::payments::payment_service_client::PaymentServiceClient;

// Example usage
fn main() {
    let client = PaymentServiceClient::new(/* channel setup here */);
    // Use the client as needed
}
```

### Health Check Module

Example of using the health check client:

```rust
use rust_grpc_client::health_check::health_client::HealthClient;

fn main() {
    let client = HealthClient::new(/* channel setup here */);
    // Perform health checks
}
```

## Development

### Updating API Definitions

- Update the `grpc-api-types` crate in the shared `../../crates/types-traits/grpc-api-types` path.
- Rebuild the project to integrate changes:

   ```bash
   cargo build
   ```

## Contributing

Contributions are welcome! Please fork the repository and submit a pull request.
