# Python Payments SDK

Calls the connector FFI layer directly from Python using protobuf-encoded bytes,
bypassing gRPC. Uses UniFFI-generated Python bindings.

## Prerequisites

- Rust toolchain (`cargo`)
- Python 3.9+
- `protoc` (Protocol Buffers compiler)

## Setup

```bash
# Build Rust lib, generate UniFFI bindings and proto stubs, build wheel
make pack
```

## Test

```bash
# Verify the packed wheel installs and the FFI layer works end-to-end
make test-pack

# With full round-trip (requires valid Stripe test key)
STRIPE_API_KEY=sk_test_your_key make test-pack
```

`test-pack` installs the wheel into an isolated temp directory and runs
`test_smoke.py`, which asserts the connector request URL and method, then
optionally exercises the full HTTP round-trip if `STRIPE_API_KEY` is set.

## Distribution

```bash
# Build wheel containing all available platform binaries (for CI / release)
make dist
# → artifacts/sdk-python/hyperswitch_payments-0.1.0.whl
```

## How it works

1. `make build-lib` — builds `crates/ffi/ffi` with `--features uniffi`
2. `make generate-bindings` — runs `uniffi-bindgen` to produce `generated/connector_service_ffi.py`
3. `make generate-proto` — runs `grpc_tools.protoc` to produce `generated/payment_pb2.py`
4. `make pack-archive` — runs `pip wheel` to produce the installable `.whl`

## ConnectorClient

`connector_client.py` provides a high-level `ConnectorClient` class that handles:
1. Serialize protobuf request to bytes
2. Call `authorize_req` via FFI to get the connector HTTP request
3. Execute the HTTP request using `requests`
4. Call `authorize_res` via FFI to parse the connector response
5. Deserialize the protobuf response
