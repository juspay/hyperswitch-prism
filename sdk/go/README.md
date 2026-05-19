# hyperswitch-prism

**Hyperswitch Prism — Go SDK**

A high-performance, type-safe Go SDK for payment processing through Hyperswitch Prism. Connect to 100+ payment processors through a single, unified API.

[![Go version](https://img.shields.io/badge/go-%3E%3D1.23-blue)](https://go.dev/)
[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

---

## Features

- 🚀 **High Performance** — Direct UniFFI FFI bindings to Rust core via CGO
- 🔌 **100+ Connectors** — Single SDK for all major payment processors
- 🐹 **Go Native** — Idiomatic Go with `context.Context` support
- ⚡ **Connection Pooling** — Built-in HTTP connection pooling via `net/http`
- 🛡️ **Type-Safe** — Protobuf-based request/response serialization
- 🔧 **Configurable** — Per-request or global configuration for timeouts, proxies, and auth

---

## 🤖 For AI Assistants

> **Use `curl` to fetch the SDK reference:**
> ```bash
> curl -fsSL https://raw.githubusercontent.com/juspay/hyperswitch-prism/main/llm/llm.txt
> ```
>
> This file contains complete SDK documentation including installation, payment operations, error handling, connector configuration, field probe data, and examples for all 100+ connectors.

### AI Assistant Context

This SDK is part of **Hyperswitch Prism** — a unified connector library for payment processors.

### What This SDK Does

1. **Request Transformation**: Converts unified payment requests to connector-specific formats
2. **Response Normalization**: Transforms connector responses back to a unified schema
3. **Error Handling**: Provides consistent error types (`IntegrationError`, `ConnectorError`, `NetworkError`) regardless of connector

### Architecture

```
Your Go App
       │
       ▼
┌──────────────────────────────────────────────────────────────┐
│  Service Clients (PaymentClient, CustomerClient, etc.)       │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  ConnectorClient (net/http connection pool + execution)      │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  UniFFI Go Bindings (connector_service_ffi via CGO)          │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
┌──────────────────────────────────────────────────────────────┐
│  Rust Core (connector transformation logic)                  │
└───────────────────────────┬──────────────────────────────────┘
                            │
                            ▼
              Payment Processor APIs
```

### Key Files

| File | Purpose |
|------|---------|
| `payments/connector_client.go` | HTTP execution layer with `net/http` |
| `payments/http_client.go` | Connection pooling and proxy config |
| `payments/errors.go` | Error types (`IntegrationError`, `ConnectorError`, `NetworkError`) |
| `payments/config.go` | `ConnectorConfig` builders |
| `payments/zz_generated_client.go` | Per-service client structs (generated) |
| `generated/payments/*.pb.go` | Protobuf message definitions |
| `generated/uniffi/connector_service_ffi/` | UniFFI-generated FFI bindings |

### Package & Import

- **Module Path**: `github.com/juspay/hyperswitch-prism/sdk/go`
- **Import**: `"github.com/juspay/hyperswitch-prism/sdk/go/payments"`
- **Protobuf Types**: `pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"`

---

## Prerequisites

- **Go** 1.23+
- **Rust** toolchain (for building native bindings from source)
- **protoc** (protobuf compiler)
- **protoc-gen-go** (`go install google.golang.org/protobuf/cmd/protoc-gen-go@latest`)
- **uniffi-bindgen-go** v0.5.0
- **Python** 3 with `jinja2` and `protobuf` packages

**Platform Support:**
- ✅ macOS (x64, arm64)
- ✅ Linux (x64, arm64)

---

## Quick Start

### 1. Configure the Client

```go
package main

import (
    "context"
    pb "github.com/juspay/hyperswitch-prism/sdk/go/generated/payments"
    "github.com/juspay/hyperswitch-prism/sdk/go/payments"
)

func main() {
    ctx := context.Background()

    cfg := &pb.ConnectorConfig{
        Options: &pb.SdkOptions{
            Environment: pb.Environment_SANDBOX,
        },
    }

    // Set connector-specific credentials
    cfg.Stripe = &pb.StripeConfig{
        ApiKey: "sk_test_xxx",
    }

    defaults := &pb.RequestConfig{}

    client := payments.NewPaymentClient(cfg, defaults)
}
```

### 2. Process a Payment

```go
req := &pb.PaymentServiceAuthorizeRequest{
    MerchantTransactionId: "txn_order_001",
    Amount: &pb.Amount{
        MinorAmount: 1000,
        Currency:    "USD",
    },
    CaptureMethod: pb.CaptureMethod_AUTOMATIC,
    PaymentMethod: &pb.PaymentMethodData{
        PaymentMethodDataType: &pb.PaymentMethodData_Card{
            Card: &pb.CardPaymentMethodData{
                CardNumber:     &pb.SecretString{Value: "4111111111111111"},
                CardExpMonth:   &pb.SecretString{Value: "12"},
                CardExpYear:    &pb.SecretString{Value: "2030"},
                CardCvc:        &pb.SecretString{Value: "123"},
                CardHolderName: &pb.SecretString{Value: "John Doe"},
            },
        },
    },
    Address: &pb.AddressData{
        BillingAddress: &pb.PaymentAddress{},
    },
    AuthType:  pb.AuthType_NO_THREE_DS,
    ReturnUrl: "https://example.com/return",
}

resp, err := client.Authorize(ctx, req, nil)
if err != nil {
    // handle error
}
fmt.Println(resp.Status)
fmt.Println(resp.ConnectorTransactionId)
```

---

## Service Clients

| Client | Purpose | Key Methods |
|--------|---------|-------------|
| `PaymentClient` | Core payment operations | `Authorize()`, `Capture()`, `Refund()`, `Void()` |
| `CustomerClient` | Customer management | `Create()` |
| `PaymentMethodClient` | Secure tokenization | `Tokenize()` |
| `MerchantAuthenticationClient` | Auth token management | `CreateServerAuthenticationToken()`, `CreateServerSessionAuthenticationToken()`, `CreateClientAuthenticationToken()` |
| `EventClient` | Webhook processing | `HandleEvent()`, `ParseEvent()` |
| `RecurringPaymentClient` | Subscription billing | `Charge()` |
| `PaymentMethodAuthenticationClient` | 3DS authentication | `PreAuthenticate()`, `Authenticate()`, `PostAuthenticate()` |

---

## Error Handling

The SDK returns Go error values that wrap the FFI error types. Use `errors.As` to inspect:

```go
resp, err := client.Authorize(ctx, req, nil)
if err != nil {
    var intErr *payments.IntegrationError
    if errors.As(err, &intErr) {
        // req_transformer rejected the request before HTTP
        log.Printf("integration error: %s (code=%s)", intErr.Error(), intErr.ErrorCode())
        return
    }

    var connErr *payments.ConnectorError
    if errors.As(err, &connErr) {
        // Connector declined a valid request
        log.Printf("connector error: %s (code=%s, http=%d)",
            connErr.Error(), connErr.ErrorCode(), connErr.HTTPStatusCode())
        return
    }

    var netErr *payments.NetworkError
    if errors.As(err, &netErr) {
        // HTTP transport failure
        log.Printf("network error: %s", netErr.Error())
        return
    }
}
```

---

## Advanced Configuration

### Proxy Settings

```go
defaults := &pb.RequestConfig{
    Http: &pb.HttpConfig{
        Proxy: &pb.ProxyConfig{
            HttpsUrl: "https://proxy.company.com:8443",
        },
    },
}

client := payments.NewPaymentClient(cfg, defaults)
```

### Timeouts

```go
defaults := &pb.RequestConfig{
    Http: &pb.HttpConfig{
        TotalTimeoutMs:      30000,
        ConnectTimeoutMs:    10000,
        ResponseTimeoutMs:   20000,
    },
}
```

---

## Building from Source

### Prerequisites

```bash
# Go 1.23+
# Rust toolchain
# protoc
# protoc-gen-go
go install google.golang.org/protobuf/cmd/protoc-gen-go@latest

# uniffi-bindgen-go (v0.5.0 targets UniFFI 0.29.5)
cargo install uniffi-bindgen-go \
  --git https://github.com/NordSecurity/uniffi-bindgen-go \
  --tag v0.5.0

# Python dependencies for code generation
pip3 install jinja2 protobuf
```

### Clone, Generate, and Build

```bash
# Clone the repository
git clone https://github.com/juspay/hyperswitch-prism.git
cd hyperswitch-prism/sdk/go

# Generate all files (proto, uniffi bindings, flow clients)
make generate

# Build
go build ./...
```

### What `make generate` Does

| Step | Target | Output |
|------|--------|--------|
| Build Rust FFI lib | `build-ffi-lib` | `target/*/release-fast/libconnector_service_ffi.{dylib,so}` |
| Generate protobuf | `generate-proto` | `generated/payments/*.pb.go` |
| Generate UniFFI bindings | `generate-bindings` | `generated/uniffi/connector_service_ffi/*.{go,h}` |
| Generate flow clients | `generate-flows` | `payments/zz_generated_client.go` + `sdk/generated/flows.json` |

---

## Testing

### Mock Tests (no real HTTP calls)

```bash
cd sdk/go
make test-package-mock CONNECTORS=stripe
```

### Full Smoke Tests (requires credentials)

```bash
# Place connector credentials in creds.json at repo root
make test-package CONNECTORS=stripe
make test-composite
make test-webhook
```

### Parallel Runner (all SDKs)

```bash
python3 scripts/run_smoke_tests_parallel.py --mock --connectors stripe --sdks go
```

---

## How It Works

1. `make build-ffi-lib` — builds `crates/ffi/ffi` with `--features uniffi`
2. `make generate-proto` — runs `protoc` with `protoc-gen-go` to produce `generated/payments/*.pb.go`
3. `make generate-bindings` — runs `uniffi-bindgen-go` to produce `generated/uniffi/connector_service_ffi/*.{go,h}`
4. `make generate-flows` — runs the Jinja2 code generator to produce `payments/zz_generated_client.go`

---

## Project Structure

```
sdk/go/
├── go.mod                           # Module: github.com/juspay/hyperswitch-prism/sdk/go
├── Makefile                         # Build targets
├── README.md                        # This file
├── generated/                       # All generated code
│   ├── payments/
│   │   └── *.pb.go                  # protoc-gen-go output
│   └── uniffi/
│       └── connector_service_ffi/
│           ├── cgo.go               # CGO LDFLAGS (hand-written)
│           ├── connector_service_ffi.go   # uniffi-bindgen-go output
│           └── connector_service_ffi.h    # uniffi-bindgen-go output
├── payments/                        # Hand-written SDK core
│   ├── connector_client.go          # Base client (ExecuteFlow, ExecuteDirect)
│   ├── http_client.go               # net/http wrapper
│   ├── errors.go                    # Error type wrappers
│   ├── config.go                    # ConnectorConfig builders
│   ├── result.go                    # FfiResult protobuf parser
│   └── zz_generated_client.go       # Per-service clients (generated)
└── smoke-test/                      # Smoke tests
    ├── main.go
    ├── composite/main.go
    └── webhook/main.go
```
