# Java/Kotlin Payments SDK

Calls the connector FFI layer directly from Kotlin/JVM using protobuf-encoded bytes,
bypassing gRPC. Uses UniFFI-generated Kotlin bindings with JNA.

## Prerequisites

- Rust toolchain (`cargo`)
- JDK 17+ (`java`, `javac`)
- `protoc` (Protocol Buffers compiler)

## Setup

```bash
# Build Rust lib, generate UniFFI bindings and proto stubs, publish to Maven Local
make pack
```

## Test

```bash
# Verify the published JAR installs and the FFI layer works end-to-end
make test-pack

# With full round-trip (requires valid Stripe test key)
STRIPE_API_KEY=sk_test_your_key make test-pack
```

`test-pack` verifies the JAR contents (classes, native lib, bindings, proto stubs),
then runs `smoke-test/` — a standalone Gradle project that depends on the published
JAR via `mavenLocal()` and asserts the connector request URL and method. Optionally
exercises the full HTTP round-trip if `STRIPE_API_KEY` is set.

## Distribution

```bash
# Build JAR containing all available platform binaries (for CI / release)
make dist
# → artifacts/sdk-java/payments-client-0.1.0.jar
```

## How it works

1. `make build-lib` — builds `crates/ffi/ffi` with `--features uniffi`
2. `make generate-bindings` — runs `uniffi-bindgen --language kotlin` to produce `generated/connector_service_ffi.kt`
3. `make generate-proto` — runs `protoc --java_out` to produce Java protobuf stubs (callable from Kotlin)
4. `make pack-archive` — runs `./gradlew jar` and copies the JAR to `artifacts/sdk-java/`

## ConnectorClient

`ConnectorClient.kt` provides a high-level client that handles:
1. Serialize protobuf request to bytes
2. Call `authorizeReq` via FFI to get the connector HTTP request
3. Execute the HTTP request using OkHttp
4. Call `authorizeRes` via FFI to parse the connector response
5. Deserialize the protobuf response
