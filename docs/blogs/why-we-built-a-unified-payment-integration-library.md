# Why we built a Unified Payment Integration Library?

If you have ever integrated a payment processor, you know the drill. You read through a PDF that was last updated in 2019, figure out what combination of API keys goes in which header, discover that "decline code 51" means something subtly different on this processor than the last one you dealt with, and then do it all over again when your business decides to add a second processor.

We have been living in this world for years building Hyperswitch, an open-source payment orchestrator. At some point we had integrations for 50+ connectors. The integrations worked well — but they were locked inside our orchestrator, not usable by anyone who just needed to talk to Stripe or Adyen without adopting an entire platform.
We always felt the Payment APIs are not more complicated than database drivers. It it just that the industry has not arrived at a standard (and it never will!!) for payments. Hence, we decided to build an open interface for Developer and AI agents to use, rather than recreate it every time.

This post is about how we did that: unbundling those integrations into a standalone library called the **Prism**, and the engineering decisions we made along the way. Some of them are genuinely interesting.

---

## Why unbundle at all?

The connector integrations inside Hyperswitch were not designed to be embedded in an orchestrator forever. They were always a self-contained layer: translate a unified request into a connector-specific HTTP call, make the call, translate the response back. The orchestrator was just the first thing to use them.

The more we looked at it, the more it seemed wrong to keep that capability locked behind a full platform deployment. If you just need to accept payments through Stripe, you should not have to adopt an orchestrator to get a well-tested, maintained integration. And if you want to switch to Adyen later, that should be a config change, not a rewrite.

So we separated the integration layer out. The result is a library with a well-defined specification — a protobuf schema covering the full payment lifecycle — that can be embedded directly in any application or deployed as a standalone service. The rest of this post is about how that works.

---

## Why protobuf for the specification?

> **Q: JSON schemas exist. OpenAPI exists. Why protobuf?**
>
> The core requirement was multi-language client generation. We needed Python developers, Java developers, TypeScript developers, and Rust developers to all be able to consume this library with first-class, type-safe APIs — without anyone hand-writing SDK code in each language. Protobuf has the most mature ecosystem for this: `prost` for Rust, `protoc-gen-java` for Java, `grpc_tools.protoc` for Python, and so on. It also doubles as our gRPC interface description when the library is deployed as a server, which turned out to be a natural fit for the two deployment modes we wanted to support (more on that below).

The specification lives in `crates/types-traits/grpc-api-types/proto/` and covers the full payment lifecycle across nine services:

| Service | What it does |
|---|---|
| `PaymentService` | Authorize, capture, void, refund, sync — the core lifecycle |
| `RecurringPaymentService` | Charge and revoke mandates for subscriptions |
| `RefundService` | Retrieve and sync refund statuses |
| `DisputeService` | Submit evidence, defend, and accept chargebacks |
| `EventService` | Process inbound webhook events |
| `PaymentMethodService` | Tokenize and retrieve payment methods |
| `CustomerService` | Create and manage customer profiles at connectors |
| `MerchantAuthenticationService` | Access tokens, session tokens, Apple Pay / Google Pay session init |
| `PaymentMethodAuthenticationService` | 3DS pre/authenticate/post flows |

Everything is strongly typed. `PaymentService.Authorize` takes a `PaymentServiceAuthorizeRequest` — amount, currency, payment method details, customer, metadata, capture method — and returns a `PaymentServiceAuthorizeResponse` with a unified status enum, connector reference IDs, and structured error details. No freeform JSON blobs. No stringly-typed status fields. The spec is the contract.

---

## The implementation: Rust at the core

> **Q: Why Rust? Wouldn't Go or Java be simpler?**
>
> A few reasons. First, we already had 50+ connector implementations in Rust from Hyperswitch, so starting there was practical. But more importantly: the library needs to be embeddable in Python, JavaScript, and Java applications without a separate process or a runtime dependency like the JVM or a Python interpreter. The only realistic way to distribute a native library that loads cleanly into all of those runtimes is as a compiled shared library — `.so` on Linux, `.dylib` on macOS. Rust produces exactly that, with no garbage collector pauses, no runtime to ship, and memory safety that does not require a GC.

The Rust codebase is organized into a handful of internal crates:

- `connector-integration` — The actual connector logic: 50+ implementations translating unified domain types into connector-specific HTTP requests and parsing responses back
- `domain_types` — Shared models: `RouterDataV2`, flow markers (`Authorize`, `Capture`, `Refund`, ...), request/response data types
- `grpc-api-types` — Rust types generated from the protobuf spec via `prost`
- `interfaces` — The trait definitions that connector implementations must satisfy

### The two-phase transformer pattern

The single most important design decision in the Rust core is that **the library never makes HTTP calls itself**. Every payment operation is split into two pure functions:

```
┌─────────────┐    req_transformer      ┌──────────────────┐
│  Unified    │ ──────────────────────▶ │ Connector HTTP   │
│  Request    │                         │ Request          │
│  (proto)    │                         │ (URL, headers,   │
└─────────────┘                         │  body)           │
                                        └────────┬─────────┘
                                                 │  you make this call
                                                 ▼
┌─────────────┐    res_transformer      ┌──────────────────┐
│  Unified    │ ◀────────────────────── │ Connector HTTP   │
│  Response   │                         │ Response         │
│  (proto)    │                         │ (raw bytes)      │
└─────────────┘                         └──────────────────┘
```

`req_transformer` takes your unified protobuf request and returns the connector-specific HTTP request — the URL, the headers, the serialized body. You make the HTTP call however you like. `res_transformer` takes the raw response bytes plus the original request and returns a unified protobuf response.

> **Q: Why not just have the library make the HTTP call for you?**
>
> Mostly because it makes the library genuinely stateless and transport-agnostic. It does not own any connection pools. It does not have opinions about TLS configuration, proxy settings, or retry logic. When this code runs inside a Python application, the Python application's `httpx` client handles the HTTP. When it runs inside the gRPC server, the server's client handles it. This also turns out to be quite testable — you can unit test transformers by feeding them request bytes and asserting on the resulting HTTP request structure, without standing up any network infrastructure.

Each flow is registered using a pair of Rust macros:

```rust
// Register the request transformer for the Authorize flow
req_transformer!(
    fn_name: authorize_req_transformer,
    request_type: PaymentServiceAuthorizeRequest,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
);

// Register the response transformer for the Authorize flow
res_transformer!(
    fn_name: authorize_res_transformer,
    request_type: PaymentServiceAuthorizeRequest,
    response_type: PaymentServiceAuthorizeResponse,
    flow_marker: Authorize,
    resource_common_data_type: PaymentFlowData,
    request_data_type: PaymentsAuthorizeData<T>,
    response_data_type: PaymentsResponseData,
);
```

The macros generate the boilerplate: connector lookup, trait object dispatch, `RouterDataV2` construction, serialization. A new flow means adding the connector trait implementation and one pair of macro invocations. The code generator (described later) handles everything else.

---

## Two ways to use it

This is where things get interesting. We wanted the library to work both as an **embedded SDK** (loaded directly into your application process) and as a **standalone gRPC service** (deployed separately, called over the network). Same Rust core, same proto types, same API — two completely different deployment topologies.

```
┌──────────────────────────────────────────────────────────┐
│                    Your Application                      │
└─────────────────────┬────────────────────────────────────┘
                      │
         ┌────────────┴────────────┐
         ▼                         ▼
 ┌──────────────┐         ┌─────────────────┐
 │   SDK Mode   │         │   gRPC Mode     │
 │  (FFI/UniFFI)│         │ (Client/Server) │
 └──────┬───────┘         └────────┬────────┘
        │                          │
        │  in-process call         │  network call
        ▼                          ▼
 ┌──────────────────────────────────────────────┐
 │          Rust Core (Prism)       │
 │  req_transformer → [HTTP] → res_transformer  │
 └──────────────────────────────────────────────┘
```

### Mode 1: The embedded SDK

In SDK mode, the Rust core compiles into a native shared library (`.so` / `.dylib`) and is exposed to host languages via **UniFFI** — Mozilla's framework for generating language bindings from Rust automatically. When your Python code calls `authorize_req_transformer(request_bytes, options_bytes)`, that call crosses the FFI boundary directly into the Rust binary running in the same process.

The FFI layer (`crates/ffi/ffi/`) is thin by design:

- `services/payments.rs` — the transformer implementations, wired to domain types via the macros above
- `handlers/payments.rs` — loads the embedded config (yes, the connector URL config is baked into the binary) and delegates to the service transformers
- `bindings/uniffi.rs` — the UniFFI bridge, where `define_ffi_flow!` exposes each flow as named FFI symbols

Data crosses the language boundary as serialized protobuf bytes. This is intentional — every language already has a protobuf runtime, so there is no custom serialization protocol to maintain, and the byte interface is completely language-neutral.

> **Q: Does this mean I need to compile Rust to use the Python SDK?**
>
> For development, yes — you run `make pack`, which builds the Rust library, runs `uniffi-bindgen` to generate the Python bindings, and packages everything into a wheel. For production use, we ship pre-built binaries for Linux x86\_64, Linux aarch64, macOS x86\_64, and macOS aarch64 inside the wheel. The loader picks the right one at runtime. You install the wheel and never think about Rust again.

### Mode 2: The gRPC server

In gRPC mode, `crates/grpc-server/grpc-server` runs as a standalone async service built on **Tonic** (Rust's async gRPC framework). It implements all nine proto services, accepts gRPC connections from any language's generated stubs, makes the connector HTTP calls internally, and returns unified proto responses over the wire.

The gRPC server calls the same Rust core transformers as the FFI layer — just from a different entry point. The transformation logic is literally the same code path. The difference is that the HTTP client lives inside the server process, not in the caller's.

Clients connect using standard gRPC stubs generated from `services.proto`. Each language SDK ships both:

```
sdk/python/
├── src/payments/           ← FFI-based embedded SDK
│   ├── connector_client.py
│   └── _generated_service_clients.py
└── grpc-client/            ← gRPC stubs for server mode

sdk/java/
├── src/                    ← FFI-based embedded SDK (JNA + UniFFI)
└── grpc-client/            ← gRPC stubs for server mode

sdk/javascript/
├── src/payments/           ← FFI-based embedded SDK (node-ffi)
└── grpc-client/            ← gRPC stubs for server mode
```

> **Q: When would you actually choose gRPC over the embedded SDK?**
>
> The embedded SDK is great when you have a single-language service and want zero network overhead — serverless functions, edge deployments, or situations where adding a sidecar is painful. The gRPC server shines in polyglot environments: if your checkout service is in Java, your fraud service is in Python, and your reconciliation job is in Go, deploying one gRPC server gives all of them a shared, consistent integration layer without each one shipping a native binary. It also gives you process isolation if that matters for your threat model.
>
> The important point is that the choice is not a migration — your `PaymentServiceAuthorizeRequest` looks identical in both modes. You change a config flag, not your application code.

| | SDK (embedded) | gRPC (network) |
|---|---|---|
| **Latency** | Microseconds (in-process) | Milliseconds (network) |
| **Deployment** | Library inside your app | Separate service to run |
| **Language support** | Python, JS, Java/Kotlin, Rust | Any language with gRPC |
| **Connector HTTP** | Your app makes the calls | Server makes the calls |
| **Best for** | Serverless, edge, single-language | Polyglot stacks, shared infra |

---

## Code generation: the glue that holds it together

Here is a problem we needed to solve: the Prism supports many payment flows (authorize, capture, void, refund, recurring charge, 3DS pre-auth, webhook handling, ...) and many SDK languages. Hand-maintaining typed client methods for each flow in each language is exactly the kind of work that introduces drift and bugs. So we do not do it.

The code generator at `sdk/codegen/generate.py` reads two sources of truth and emits all the SDK client boilerplate automatically.

> **Q: What are the two sources of truth?**
>
> 1. `services.proto` compiled to a binary descriptor — this tells the generator every RPC name, its request type, its response type, and its doc comment.
> 2. `crates/ffi/ffi/src/services/payments.rs` — this tells the generator which flows are actually implemented, by scanning for `req_transformer!` invocations.
>
> The generator takes their intersection. A flow in proto but not implemented in Rust? Warning, skipped — we don't ship unimplemented APIs. A transformer in Rust with no matching proto RPC? Also a warning — the spec is the authority, not the implementation.

Running `make generate` produces:

**In Rust** (`crates/ffi/ffi/src/`):
- `_generated_flow_registrations.rs` — the `impl_flow_handlers!` wiring for each flow
- `_generated_ffi_flows.rs` — the `define_ffi_flow!` UniFFI exposure for each flow

**In Python** (`sdk/python/src/payments/`):
- `_generated_service_clients.py` — per-service typed client classes:
  ```python
  class PaymentClient(_ConnectorClientBase):
      async def authorize(self, request: PaymentServiceAuthorizeRequest, options=None) -> PaymentServiceAuthorizeResponse:
          """PaymentService.Authorize — Authorizes a payment amount on a payment method..."""
          return await self._execute_flow("authorize", request, _pb2.PaymentServiceAuthorizeResponse, options)
  ```
- `connector_client.pyi` — type stubs so Pylance and mypy see typed signatures without running any code

**In TypeScript** (`sdk/javascript/src/payments/`):
- `_generated_connector_client_flows.ts` — per-service typed async client classes
- `_generated_uniffi_client_flows.ts` — typed wrappers around the raw FFI byte calls

**In Kotlin** (`sdk/java/src/main/kotlin/`):
- `GeneratedFlows.kt` — a `FlowRegistry` object mapping flow names to UniFFI-generated Kotlin function references, plus per-service client classes:
  ```kotlin
  class PaymentClient(config: ConnectorConfig, ...) : ConnectorClient(config, ...) {
      fun authorize(request: PaymentServiceAuthorizeRequest, options: RequestConfig? = null): PaymentServiceAuthorizeResponse =
          executeFlow("authorize", request.toByteArray(), PaymentServiceAuthorizeResponse.parser(), options)
  }
  ```

The generator also handles a second category of flows: **single-step flows** (like webhook processing) that transform a request directly into a response without an HTTP round-trip. These get a `_execute_direct` path instead of the two-phase req/HTTP/res path.

Here is the full pipeline:

```
services.proto
    │
    ├── prost (Rust build.rs)           → grpc-api-types crate (Rust types)
    ├── grpc_tools.protoc               → payment_pb2.py (Python proto stubs)
    ├── protoc-gen-java                 → Payment.java (Java/Kotlin proto stubs)
    ├── protoc (JS plugin)              → proto.js / proto.d.ts (JS proto stubs)
    └── protoc (binary descriptor)      → services.desc
                                                │
crates/ffi/ffi/src/services/payments.rs ──────────┤
    (req/res transformer registrations)         │
                                                ▼
                                          generate.py
                                                │
              ┌─────────────────────────────────┼──────────────────────────────┐
              ▼                                 ▼                              ▼
  _generated_ffi_flows.rs         _generated_service_clients.py     GeneratedFlows.kt
  _generated_flow_registrations.rs connector_client.pyi             _generated_connector_client_flows.ts
                                  _generated_flows.py               _generated_uniffi_client_flows.ts


cargo build --features uniffi
    └── uniffi-bindgen
              │
              ├── connector_service_ffi.py   (Python native bindings)
              ├── ConnectorServiceFfi.kt     (Kotlin/JVM native bindings)
              └── ffi.js                     (Node.js native bindings)
```

The practical result: add a new flow to `services.proto`, implement the transformer pair in Rust, run `make generate` — and every language SDK gets a typed, documented method for that flow. No one writes boilerplate by hand.

---

## Walking through a real authorize call

Let's trace what actually happens when a Python application calls `client.authorize(...)` in SDK mode. This makes the layering concrete.

```
① App builds PaymentServiceAuthorizeRequest (protobuf message)

② PaymentClient.authorize() → _execute_flow("authorize", request, ...)

③ _ConnectorClientBase._execute_flow():

   a. request.SerializeToString() → request_bytes

   b. authorize_req_transformer(request_bytes, options_bytes)
      ──── FFI boundary: Python → Rust shared library ────
      Rust: build_router_data! macro
        ├── ConnectorEnum::from("stripe")   ← look up connector
        ├── connector.get_connector_integration_v2()
        ├── proto bytes → PaymentFlowData + PaymentsAuthorizeData
        ├── construct RouterDataV2 { flow, request, auth, ... }
        └── connector.build_request(router_data) → Request { url, headers, body }
      serialize Request → FfiConnectorHttpRequest bytes
      ──── returns bytes across FFI boundary ────

   c. deserialize FfiConnectorHttpRequest → url, method, headers, body

   d. httpx AsyncClient.post(url, headers=headers, content=body)
      ← this is the actual outbound HTTP call to Stripe

   e. raw response bytes received

   f. authorize_res_transformer(response_bytes, request_bytes, options_bytes)
      ──── FFI boundary: Python → Rust shared library ────
      Rust: connector.handle_response(raw_bytes)
        ├── parse Stripe's JSON response format
        └── map → PaymentServiceAuthorizeResponse (unified proto)
      serialize → proto bytes
      ──── returns bytes across FFI boundary ────

   g. PaymentServiceAuthorizeResponse.FromString(bytes)

④ App receives unified PaymentServiceAuthorizeResponse
```

In gRPC mode, steps ③b through ③f happen inside the `grpc-server` process. The app sends the protobuf request over the network and gets the protobuf response back. The connector lookup, HTTP call, and response transformation are identical — just running in a different process.

---

## Where we go from here — together

We want to be upfront about what this is and what it is not.

What it is: a working implementation with 50+ connectors, a protobuf specification that covers the full payment lifecycle, and SDKs in four languages. It is ready to use today.

What it is not: a finished standard. The spec reflects our understanding of what payment integrations need to look like. That understanding is incomplete, and we know it. Payment APIs have a very long tail of edge cases — 3DS flows that differ between processors, webhook schemas that change without notice, authorization responses that technically succeeded but should be treated as soft declines. There is no team small enough to have seen all of it.

That is why community ownership matters here, not as a marketing posture, but as a practical necessity.

**If you want to use it:** install the SDK, run `make generate` to see what flows are available, and point it at your test credentials. When something breaks — and something will — open an issue. The more connectors and flows get exercised in real environments, the faster the rough edges get found.

**If you want to contribute a connector:** implement a Rust trait in `connector-integration/`. The FFI layer, gRPC server, and all language SDKs pick it up automatically. You do not need to write Python or JavaScript or maintain anything outside that one crate.

**If you want to contribute a flow:** start with a discussion on the `services.proto` shape — that is the community contract, so it deserves a conversation before code gets written. Once there is agreement, implement the transformer pair in Rust, run `make generate`, and every SDK gets the new method in every language.

**If you disagree with a spec decision:** open a discussion. The whole point of making this community-owned is that no single team's assumptions should be baked in permanently. If you have seen payment edge cases that the current schema cannot express, that is exactly the kind of feedback that shapes a standard.

The longer arc here is for `services.proto` to evolve into something the payments community — developers, processors, orchestrators, and everyone else in the stack — maintains collectively. The same way OpenTelemetry's semantic conventions emerged from broad input, not from one company's opinions. The same way JDBC worked because it was simple enough to implement and strict enough to actually abstract.

