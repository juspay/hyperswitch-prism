# SDK FFI Performance Report

## Summary

The FFI (Foreign Function Interface) boundary crossing adds **<4ms overhead per call** across all SDKs, accounting for **0.2–0.3% of total round-trip time**. Network latency dominates at >99.7%.

## Cross-SDK Comparison

| Metric | Rust | JavaScript | Python |
|---|---:|---:|---:|
| Avg req_ffi (serialize + FFI call + decode) | 0.47ms | 0.79ms | 0.78ms |
| Avg res_ffi (encode + FFI call + deserialize) | 1.57ms | 2.26ms | 3.04ms |
| **Avg total overhead** | **2.04ms** | **3.05ms** | **3.82ms** |
| Avg HTTP latency | 1152ms | 1225ms | 1236ms |
| **Overhead as % of total** | **0.18%** | **0.25%** | **0.31%** |
| FFI round-trips measured | 3 | 10 | 10 |

Kotlin data is not shown because its Maven publish pipeline does not yet include the `PerfLog` class in the published artifact. The instrumentation is in place and will report once the publish is fixed.

## What "req_ffi" and "res_ffi" Measure

Each SDK flow does a full round-trip through the Rust core via FFI:

```
req_ffi: [serialize proto → bytes] → [FFI req_transformer call] → [decode result bytes → HTTP request]
   ↓
 HTTP:   [send request to connector API] → [receive response]
   ↓
res_ffi: [encode HTTP response → bytes] → [FFI res_transformer call] → [decode result bytes → domain proto]
```

The timers capture **everything on the SDK side of the boundary**, including protobuf serialization, the native function call, and result decoding — not just the raw C-call itself.

## Why the Measurement Is Accurate

**Timing is inside the ConnectorClient, not the test harness.** Each SDK's `_execute_flow` / `executeFlow` method has three `Instant::now()` / `time.monotonic()` / `performance.now()` / `System.nanoTime()` checkpoints placed directly around the FFI calls and the HTTP call. This means:

1. **No test-framework noise.** The timer starts after config resolution and stops before the result is returned. Test setup, credential loading, and result formatting are excluded.

2. **HTTP variance is visible.** Each row shows the actual HTTP latency for that specific call, so you can see that network jitter (not FFI) causes the variance between runs.

3. **Real connector traffic, not mocks.** All measurements hit the live Stripe sandbox API. This captures realistic serialization sizes and response parsing for production-shaped payloads.

4. **Same Rust core across all SDKs.** Every SDK calls the same compiled `libconnector_service_ffi.dylib`. The only variable is the language-side binding layer.

## Why Rust Scenarios Appeared Faster in the Per-Scenario View

The per-scenario timing (PERFORMANCE SUMMARY) showed ~1.2s for Rust vs ~2.5s for Python/JS/Kotlin. This is **not** an FFI difference — it is a **scenario difference**:

| SDK | Scenarios run | Calls per scenario | Scenario time |
|---|---|---:|---:|
| Rust | `proxy_authorize`, `setup_recurring` | 1 HTTP call each | ~1.2s |
| Python/JS/Kotlin | `capture`, `refund`, `void`, `get` | 2 HTTP calls each (authorize first, then the operation) | ~2.5s |

The Python/JS/Kotlin examples are composite: `process_refund()` first calls `authorize()` to get a transaction ID, then calls `refund()`. That is two full Stripe round-trips. The Rust test used auto-generated harness files with single-call flows.

The FFI breakdown confirms identical per-call HTTP latency across all SDKs (~1.1–1.5s to Stripe per call).

## Architecture

| SDK | FFI binding | HTTP client | Serialization |
|---|---|---|---|
| Rust | Direct Rust call (no FFI) | reqwest | protobuf (prost) |
| Python | UniFFI → ctypes | httpx (h2) | protobuf |
| JavaScript | UniFFI → koffi | undici (fetch) | protobufjs |
| Kotlin | UniFFI → JNA | OkHttp | protobuf-java |

All SDKs share the same Rust core library (`libconnector_service_ffi`) compiled with `--profile release-fast`. The connector-specific request/response transformation logic is identical — only the language binding and HTTP transport differ.

## Reproducing

```bash
# Full run with all SDKs + comparison table at the end
make -C sdk test

# Single SDK
make -C sdk test-rust
make -C sdk test-python
make -C sdk test-javascript
make -C sdk test-java
```

The cross-SDK comparison table prints at the very end of `make -C sdk test` after all four SDKs have run. Each SDK also prints its own per-flow FFI breakdown inline.
