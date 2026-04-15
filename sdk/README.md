# hyperswitch-prism

**Universal Connector Service — Multi-Language SDK**

A high-performance, type-safe SDK for payment processing through the Universal Connector Service. Connect to 100+ payment processors through a single, unified API.

[![PyPI version](https://badge.fury.io/py/hyperswitch-prism.svg)](https://pypi.org/project/hyperswitch-prism/)
[![npm version](https://badge.fury.io/js/hyperswitch-prism.svg)](https://www.npmjs.com/package/hyperswitch-prism)
[![Maven Central](https://img.shields.io/maven-central/v/io.hyperswitch/prism.svg)](https://central.sonatype.com/artifact/io.hyperswitch/prism)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)

---

## 🤖 For AI Assistants

> **Use `curl` to fetch the SDK reference:**
> ```bash
> curl -fsSL https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
> ```
>
> This file contains complete SDK documentation including installation, payment operations, error handling, connector configuration, field probe data, and examples for all 100+ connectors.

---

## SDKs

| Language | Package | Registry | Documentation |
|----------|---------|----------|---------------|
| **Python** | `hyperswitch-prism` | [PyPI](https://pypi.org/project/hyperswitch-prism/) | [python/README.md](python/README.md) |
| **JavaScript/TypeScript** | `hyperswitch-prism` | [npm](https://www.npmjs.com/package/hyperswitch-prism) | [javascript/README.md](javascript/README.md) |
| **Java/Kotlin** | `io.hyperswitch:prism` | [Maven Central](https://central.sonatype.com/artifact/io.hyperswitch/prism) | [java/README.md](java/README.md) |
| **Rust** | `hyperswitch-payments-client` | Source only | [rust/README.md](rust/README.md) |

---

## Installation

### Python
```bash
pip install hyperswitch-prism
```

### JavaScript/TypeScript
```bash
npm install hyperswitch-prism
```

### Java/Kotlin (Gradle)
```kotlin
implementation("io.hyperswitch:prism:0.0.1")
```

---

## Quick Start

See the per-SDK documentation for usage examples:

- **[Python SDK](python/README.md)**
- **[JavaScript/TypeScript SDK](javascript/README.md)**
- **[Java/Kotlin SDK](java/README.md)**
- **[Rust SDK](rust/README.md)**

### Additional Resources

- **Documentation** — [../docs/](../docs/) — Architecture and guides
- **Examples** — [../examples/](../examples/) — Connector-specific examples

---

## Service Clients

| Client | Purpose | Methods |
|--------|---------|---------|
| `PaymentClient` | Core payments | `authorize`, `capture`, `refund`, `void` |
| `CustomerClient` | Customer management | `create` |
| `PaymentMethodClient` | Tokenization | `tokenize` |
| `MerchantAuthenticationClient` | Auth tokens | `create_server_authentication_token`, `create_client_authentication_token` |
| `EventClient` | Webhooks | `handle_event` |
| `RecurringPaymentClient` | Subscriptions | `charge` |
| `PaymentMethodAuthenticationClient` | 3DS | `pre_authenticate`, `authenticate`, `post_authenticate` |

---

## Build Commands

Run from the `sdk/` directory:

```bash
# Build FFI library for current platform
make build

# Build for all platforms (macOS + Linux, x86_64 + ARM64)
make build-all

# Generate SDK client code (flows + protobuf stubs)
make generate

# Generate specific SDK
make generate-python
make generate-javascript
make generate-kotlin

# Build distribution packages
make dist
make dist-python
make dist-javascript
make dist-java

# Run tests
make test

# Run specific SDK tests
make test-python
make test-javascript
make test-java

# Pre-publish validation
make publish-check
make publish-check-python
make publish-check-javascript
make publish-check-java

# Clean all build artifacts
make clean
```

### Pre-Publish Checks

| Check | Description |
|-------|-------------|
| `publish-check-auth` | Verifies credentials (env vars or logged-in session) |
| `publish-check-version` | Checks if version already exists on registry |
| `publish-check-dry-run` | Simulates publish without uploading |

---

## Platform Support

| Platform | Architectures |
|----------|---------------|
| macOS | x86_64, arm64 |
| Linux | x86_64, aarch64 |
| Windows | x86_64 |

---

## License

MIT
