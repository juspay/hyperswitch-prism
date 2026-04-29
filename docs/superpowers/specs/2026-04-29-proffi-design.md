# proffi — Protobuf-defined FFI on top of UniFFI

**Status:** Design accepted, ready for implementation planning
**Date:** 2026-04-29
**Origin:** Extraction of the protobuf-as-FFI-envelope pattern in `hyperswitch-prism` into a standalone, domain-agnostic project at `~/src/proffi`.

---

## 1. Motivation

`hyperswitch-prism` ships a Rust connector library to multiple host languages
(Python, Kotlin, JavaScript) by combining UniFFI with a uniform protobuf
envelope. Every UniFFI-exported function has the shape `Vec<u8> -> Vec<u8>`,
where the bytes are prost-encoded request and response protos. Per-language
SDKs use `protoc`-generated message types and a thin wrapper that handles the
encode-call-decode cycle.

This collapses two normally distinct problems into one:

1. **UniFFI's per-language type-mapping surface** shrinks to a single byte-vector
   ABI, eliminating most of the per-language idiomatic-binding work.
2. **Languages UniFFI does not natively target** (notably JavaScript) become
   reachable: any language with a protobuf code generator and an FFI loader can
   call into the cdylib using a small, fixed set of C-ABI bindings.

The pattern is not specific to payments. Any project shipping a Rust library to
many languages — CLI tools, ML inference helpers, format converters, simulators
— can benefit. This spec extracts the pattern into a reusable, well-bounded
project named **proffi**.

## 2. Goals and non-goals

### Goals

- Let a developer declare an FFI surface as a normal `.proto` `service { rpc … }`
  and implement the rpc handlers as plain sync Rust functions.
- Generate the UniFFI exports, byte envelope, and per-language thin wrappers
  automatically. The user writes no `Vec<u8>` plumbing by hand.
- Catch handler/proto mismatches at build time, before any binary ships.
- Support Python, Kotlin, and JavaScript (via [koffi](https://koffi.dev)) as the
  MVP language set.
- Keep the runtime surface small enough that a competent reader can hold it in
  their head — proffi should be transparent infrastructure, not a framework.

### Non-goals (MVP)

- Async handlers. Sync only. Async UniFFI exports add complexity across all four
  runtimes (Rust, Python, Kotlin, JS) and the prism use case proves sync is
  sufficient. Deferred to a later release behind an opt-in attribute.
- Streaming RPCs (`stream` keyword). Same rationale — meaningful complexity tax
  for limited near-term value.
- Cross-compilation orchestration (`build-macos-universal`, `cargo zigbuild`,
  etc.). Users invoke `cargo build --target` themselves; the CLI consumes
  whichever cdylib they point it at.
- Packaging templates (Python wheels, npm tarballs, gradle modules, podspecs).
  The CLI emits loose source files; users wrap them in their own packaging.
- Swift, Ruby, Go, C# targets. UniFFI supports several of them; deferred to
  prove the pattern on three first.

## 3. User-facing model

### 3.1 What the user writes

A user adopting proffi authors three things:

**`proto/greeter.proto`:**

```proto
syntax = "proto3";
package greeter;

service Greeter {
  rpc SayHello(HelloRequest) returns (HelloResponse);
}

message HelloRequest  { string name = 1; }
message HelloResponse { string message = 1; }
```

**`Cargo.toml`:**

```toml
[lib]
crate-type = ["cdylib"]

[dependencies]
proffi = "0.1"
prost  = "0.14"

[build-dependencies]
proffi-build = "0.1"
```

**`src/lib.rs`:**

```rust
proffi::setup!();

mod proto { include!(concat!(env!("OUT_DIR"), "/greeter.rs")); }

#[proffi::rpc(service = "greeter.Greeter", method = "SayHello")]
fn say_hello(req: proto::HelloRequest) -> Result<proto::HelloResponse, proffi::Error> {
    Ok(proto::HelloResponse {
        message: format!("hello, {}", req.name),
    })
}
```

**`build.rs`:**

```rust
fn main() {
    proffi_build::compile(&["proto/greeter.proto"], &["proto"]).unwrap();
}
```

That is the entire authoring surface. No `define_ffi_flow!`, no `FfiResult`
plumbing, no manual `#[uniffi::export]` annotations, no `Vec<u8>` decode/encode.

### 3.2 What the user runs

```bash
cargo build --release                              # produces libgreeter.dylib + manifest.json
proffi generate --lang python     --out ./py-sdk   # generates typed Python wrapper
proffi generate --lang kotlin     --out ./kt-sdk
proffi generate --lang javascript --out ./js-sdk
```

Each generated SDK exposes typed RPC methods namespaced per-service. Calling
into the FFI from any language looks roughly the same:

```python
client.greeter.say_hello(HelloRequest(name="world")).message
```

```kotlin
client.greeter.sayHello(HelloRequest(name="world")).message
```

```javascript
client.greeter.sayHello({ name: "world" }).message
```

### 3.3 Decisions baked into this surface

| Decision | Choice | Rationale |
|---|---|---|
| Authoring model | Free-form RPC (any `service.rpc`) | More general than prism's req/res transformer pattern; that pattern can be modeled on top trivially. |
| Deliverable shape | Runtime crate + codegen CLI | Runtime makes the pattern usable; CLI removes per-rpc boilerplate while staying out of full SDK packaging. |
| Target languages | Python + Kotlin + JavaScript | Two UniFFI-native targets plus one non-UniFFI target (JS via koffi) proves the protobuf-extends-UniFFI thesis. |
| Error model | Hybrid — `proffi.Error` for system failures, domain errors live inside user response protos | Keeps proffi opinion-free about domain modeling; matches gRPC convention. |
| Handler discovery | `#[proffi::rpc]` attribute macro | Idiomatic Rust, code-local, grep-able; avoids name/path coupling. |
| Async | Sync-only in MVP | Async is a meaningful complexity tax across four runtimes; prism proves sync is enough for the core use case. |
| Codegen architecture | Manifest-bridged compile-time JSON | Catches typos and drift at build time without requiring a successful binary build first. |

## 4. Architecture

### 4.1 Repo layout

`~/src/proffi/` is a Cargo workspace:

```
proffi/
├── Cargo.toml                      # workspace root
├── crates/
│   ├── proffi/                     # public re-export crate (what users add to Cargo.toml)
│   ├── proffi-macros/              # proc-macro crate — #[proffi::rpc], proffi::setup!
│   ├── proffi-runtime/             # Vec<u8>->Vec<u8> runners, FfiResult envelope, prost helpers
│   ├── proffi-build/               # build.rs helper: prost-build wrapper + manifest aggregator
│   └── proffi-cli/                 # binary: `proffi generate|check|list`
├── proto/
│   └── proffi.proto                # well-known FfiResult, proffi.Error
├── templates/                      # jinja2 templates (consumed via minijinja or tera)
│   ├── python/
│   ├── kotlin/
│   └── javascript/
├── examples/
│   └── greeter/                    # full end-to-end: proto + Rust handler + 3 SDKs + smoke tests
├── docs/
│   └── 2026-04-29-design.md        # this document, copied into the new repo
└── README.md
```

### 4.2 Crate boundaries

Each crate has one responsibility and exposes a clean public API:

| Crate | Owns | Depends on |
|---|---|---|
| `proffi-runtime` | `Error`, `FfiResult`, `run<Req, Res>(input, handler)` runner, panic catch, prost decode/encode helpers. | `prost`, `bytes`. |
| `proffi-macros` | `#[proffi::rpc]`, `proffi::setup!` proc macros. Generates UniFFI export wrapper around `proffi-runtime::run`. Writes per-rpc JSON to `OUT_DIR`. | `syn`, `quote`, `proc-macro2`, `serde_json`. |
| `proffi-build` | `compile(protos, includes)` (wraps `prost-build`, captures descriptor), `finalize()` (aggregates per-rpc JSON, validates against descriptor, writes manifest.json). | `prost-build`, `prost-types`, `serde_json`. |
| `proffi` | Re-exports of `proffi-runtime` types, `proffi-macros` macros, and a pinned `pub use uniffi`. The single dependency users add. | `proffi-runtime`, `proffi-macros`, `uniffi` (pinned). |
| `proffi-cli` | `proffi generate`, `proffi check`, `proffi list`. Reads manifest + descriptor, renders templates, calls `uniffi_bindgen::bindings::{python, kotlin}::generate` as library functions. | `clap`, `serde_json`, `prost-types`, `uniffi_bindgen`, `minijinja` (or `tera`). |

The split exists because:

- proc-macro crates cannot export non-macro items, so runtime types must live
  separately.
- `proffi-build` is a library called from the user's `build.rs`, not a CLI; it
  must be a separate crate to avoid pulling proc-macro deps into build scripts.
- `proffi-cli` is heavyweight (uniffi-bindgen, template engine); keeping it out
  of the runtime path is essential.

### 4.3 Boundary checks

For each crate one should be able to answer: what does it do, how do you use
it, what does it depend on. The split above is the right granularity to keep
each crate small and focused. A reader can understand `proffi-runtime` without
reading any other crate; consumers of `proffi-runtime` can be tested
independently of macros and CLI.

## 5. Wire format

The bytes flowing across every FFI call follow a single shape, defined in
`proto/proffi.proto`:

```proto
syntax = "proto3";
package proffi;

import "google/protobuf/any.proto";

message Error {
  string code = 1;          // reserved system codes (see below)
  string message = 2;
  google.protobuf.Any details = 3;   // optional; user-attachable
}

message FfiResult {
  oneof payload {
    bytes ok  = 1;          // user-defined response proto, prost-encoded
    Error err = 2;
  }
}
```

`bytes ok` carries a separately-encoded message rather than `google.protobuf.Any`
because `Any` would force the runtime to know type URLs for every user proto.
Carrying raw bytes keeps the runtime fully generic — the per-language wrapper
knows the expected response type for each rpc and decodes the bytes directly.

### 5.1 Reserved system error codes

System failures are surfaced via `proffi.Error.code`:

- `DECODE_FAILED` — input bytes did not parse as the expected request proto.
- `HANDLER_PANIC` — the handler panicked; the runner caught it.
- `INTERNAL` — anything else attributable to the FFI runtime itself.

Domain errors are not proffi's concern. Users model them inside their own
response proto using a `oneof` (e.g. `oneof outcome { Success success; BusinessError error; }`).
This keeps the transport envelope independent of any domain.

### 5.2 The runner

`proffi-runtime::run` is the single function every generated FFI export calls:

```rust
pub fn run<Req, Res, F>(input: Vec<u8>, handler: F) -> Vec<u8>
where
    Req: prost::Message + Default,
    Res: prost::Message,
    F: FnOnce(Req) -> Result<Res, crate::Error>,
{
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let req = Req::decode(&*input).map_err(Error::decode)?;
        handler(req)
    }));

    let envelope = match result {
        Ok(Ok(res))  => FfiResult { payload: Some(Payload::Ok(res.encode_to_vec())) },
        Ok(Err(e))   => FfiResult { payload: Some(Payload::Err(e.into())) },
        Err(panic)   => FfiResult { payload: Some(Payload::Err(Error::from_panic(panic))) },
    };
    envelope.encode_to_vec()
}
```

### 5.3 Macro expansion

`#[proffi::rpc]` expands into a UniFFI-exported byte-in/byte-out function plus
a renamed copy of the user's original handler:

```rust
#[uniffi::export]
pub fn say_hello(input: Vec<u8>) -> Vec<u8> {
    proffi::runtime::run::<proto::HelloRequest, proto::HelloResponse, _>(input, |req| {
        __proffi_user_say_hello(req)
    })
}
fn __proffi_user_say_hello(req: proto::HelloRequest)
    -> Result<proto::HelloResponse, proffi::Error> { /* user body */ }
```

The macro also writes `$OUT_DIR/proffi-rpc-{uuid}.json`:

```json
{
  "service": "greeter.Greeter",
  "method": "SayHello",
  "rust_fn": "say_hello",
  "req_type": "greeter.HelloRequest",
  "res_type": "greeter.HelloResponse",
  "source_location": "src/lib.rs:42"
}
```

### 5.4 JS-via-koffi specifics

UniFFI exposes `Vec<u8>` over the C ABI as a `RustBuffer`:

```c
struct RustBuffer { uint8_t *data; uint64_t len; uint64_t capacity; };
```

The koffi-based JS wrapper performs a fixed dance per call: allocate a
`RustBuffer` from a JS Buffer, invoke the export, copy `data[..len]` out, and
call `ffi_<crate>_rust_buffer_free`. This plumbing is constant — three or four
function bindings total — regardless of how many RPCs the user defines, because
every export has the same `RustBuffer -> RustBuffer` shape. The proffi `js`
template owns the `RustBuffer` struct definition and the per-call dance once;
generated per-rpc methods are pure data marshalling.

## 6. Build flow

```
                ┌─────────────────────────────────────┐
                │  user crate                         │
                │  ├── proto/*.proto                  │
                │  ├── src/lib.rs (#[proffi::rpc])    │
                │  └── build.rs                       │
                └──────────────────┬──────────────────┘
                                   │  cargo build
                                   ▼
        ┌──────────────────────────────────────────────────┐
        │ build.rs  → proffi_build::compile()              │
        │   1. prost-build → Rust types from .proto        │
        │   2. write descriptor.bin to target/proffi/...   │
        └──────────────────────────────────────────────────┘
                                   │
                                   ▼
        ┌──────────────────────────────────────────────────┐
        │ #[proffi::rpc] macro (during rustc)              │
        │   - generates UniFFI export wrapper              │
        │   - writes proffi-rpc-{uuid}.json to OUT_DIR     │
        └──────────────────────────────────────────────────┘
                                   │
                                   ▼
        ┌──────────────────────────────────────────────────┐
        │ proffi_build::finalize()  (end of build.rs)      │
        │   - aggregates per-rpc JSONs from OUT_DIR        │
        │   - validates against descriptor:                │
        │       * every #[rpc] resolves to a real rpc      │
        │       * every rpc is covered (configurable)      │
        │       * Req/Res Rust paths align with proto      │
        │   - writes target/proffi/<crate>/manifest.json   │
        │   - hard-fails build on mismatch                 │
        └──────────────────────────────────────────────────┘
                                   │
                                   ▼
              libfoo.dylib  +  manifest.json  +  descriptor.bin
                                   │
                       proffi CLI  │
                                   ▼
        ┌──────────────────────────────────────────────────┐
        │  proffi generate --lang <X> --out <dir>          │
        │  - reads manifest.json + descriptor.bin          │
        │  - calls uniffi_bindgen::bindings::<X>::generate │
        │    as a library function (no external binaries)  │
        │  - runs protoc for user-language proto stubs     │
        │  - renders client.{py,kt,js} from templates      │
        └──────────────────────────────────────────────────┘
```

### 6.1 Validation timing

Validation runs inside `proffi_build::finalize()`, called from the user's
`build.rs`. A user who runs only `cargo build` — without ever invoking the CLI
— still gets every guarantee about handler ↔ proto alignment. The CLI is only
needed for foreign-language wrappers; the Rust crate alone is fully
self-validating.

### 6.2 Manifest path

The aggregated manifest is written to:

```
${CARGO_TARGET_DIR:-target}/proffi/<crate-name>/manifest.json
```

- `CARGO_TARGET_DIR` is honored automatically (Cargo standard env var).
- The `<crate-name>` segment handles workspaces with multiple FFI cdylibs
  cleanly — each cdylib owns its own manifest, no races, no collisions.
- Lives under `target/`, so `cargo clean` purges it without hand-cleaning.

`OUT_DIR` is **not** an option for the aggregated manifest: it is per-build,
per-profile, and contains a content hash, so external tools (the CLI) cannot
reliably locate it. Per-rpc JSON entries do live in `OUT_DIR` because they are
build-script-private intermediates — that is correct and matches `prost-build`,
`tonic-build`, `bindgen`, and other codegen build scripts.

Override knobs for unusual setups:

```rust
proffi_build::Config::default()
    .manifest_dir("/custom/path")
    .compile(&["proto/greeter.proto"], &["proto"])?;
```

Plus a `PROFFI_MANIFEST_DIR` env var that the build helper and the CLI both
honor. No `OUT_DIR` override — it cannot serve the use case.

### 6.3 CLI surface (MVP)

```
proffi generate --lang <python|kotlin|javascript> --out <dir>
  Reads manifest.json + descriptor.bin from target/proffi/<crate>/.
  Resolves the cdylib via --lib-path or env (PROFFI_LIB_PATH).
  Emits a typed wrapper package for the chosen language.

proffi check
  Same validation as build.rs finalize, runnable standalone.
  Useful for CI: "no #[rpc] without a proto rpc, no proto rpc without #[rpc]".

proffi list
  Pretty-prints the manifest: which RPCs, which Rust paths, which proto types.
  Useful for "what does this dylib actually expose?" debugging.
```

### 6.4 Failure modes (explicit)

1. **Handler points to a non-existent rpc.**
   `finalize()` fails with:
   `"#[proffi::rpc(service=\"x.Y\", method=\"Z\")] in src/lib.rs:42 — no such rpc \"Z\" in service \"x.Y\""`.

2. **Proto rpc has no handler.**
   Configurable. Default = error in release builds, warning in dev. Override
   via `Config::default().allow_partial(true)`.

3. **Type mismatch between handler and proto.**
   `finalize()` fails with:
   `"#[proffi::rpc(service=\"x.Y\", method=\"Z\")]: handler returns proto::Foo but rpc Z's output_type is x.Bar"`.

## 7. Per-language wrapper generation

Every language wrapper does the same three things: (1) load the cdylib + bind
the FFI exports, (2) for each rpc, expose a typed method that proto-encodes
input and proto-decodes output, (3) translate `proffi.Error` into the
language's idiomatic error type. The shape of (2) is namespaced per service
(e.g. `client.greeter.say_hello`), mirroring grpc-tools-generated clients.

### 7.1 Python (`--out py-sdk/`)

```
py-sdk/
├── greeter_proffi/
│   ├── __init__.py            # re-exports Client class
│   ├── _ffi.py                # UniFFI-generated bindings (delegated to in-process bindgen)
│   ├── _proto/                # protoc --python_out output
│   └── client.py              # typed wrapper (per-service classes)
└── pyproject.toml             # minimal — users replace as needed
```

Generation:
- `uniffi_bindgen::bindings::python::generate(library_path, crate_name, out)` —
  in-process Rust call from the CLI; no external `uniffi-bindgen-python`
  required.
- `protoc --python_out=...` for `_proto/`.
- `client.py` rendered from the manifest via templates.

A method body:

```python
class GreeterService:
    def say_hello(self, req: HelloRequest) -> HelloResponse:
        out = _ffi.say_hello(req.SerializeToString())
        result = FfiResult.FromString(bytes(out))
        if result.WhichOneof("payload") == "err":
            raise ProffiError(result.err.code, result.err.message)
        return HelloResponse.FromString(result.ok)
```

### 7.2 Kotlin (`--out kt-sdk/`)

Same pattern via `uniffi_bindgen::bindings::kotlin::generate` (library call) and
`protoc --kotlin_out`. Generated `Client.kt` exposes per-service classes with
typed methods; throws `ProffiException` on `Error`.

### 7.3 JavaScript (`--out js-sdk/`)

UniFFI does not target JavaScript, so the generator emits a koffi-based binding
directly. Surface remains small because every export is `bytes_in -> bytes_out`:

```
js-sdk/
├── package.json
├── src/
│   ├── ffi.js              # koffi.load + RustBuffer struct + per-rpc ABI calls
│   ├── proto/              # protoc-gen-js / ts-proto output
│   └── client.js           # typed methods, per-service namespacing
└── README.md
```

The fixed koffi plumbing (`RustBuffer` struct, `ffi_<crate>_rust_buffer_free`,
allocate-call-copy-free dance) is template-rendered once per generated SDK.
Per-rpc methods are generated from the manifest.

### 7.4 UniFFI bindgen as a library — design alternative resolved

The natural assumption is that the CLI shells out to external
`uniffi-bindgen-python` / `uniffi-bindgen-kotlin` binaries. We do **not** do
that, for these reasons:

1. **Version pinning.** The bindgen tool's metadata format must match the
   `uniffi` runtime version compiled into the cdylib. An installed-on-PATH
   bindgen drifts; users must remember to upgrade two things in lockstep.
2. **Crate-name introspection.** Bindgen needs the user crate's name to find
   metadata symbols inside the dylib. A project-local bindgen binary hardcodes
   it; PATH bindgens require an explicit flag.
3. **PATH brittleness.** Each new dev/CI environment needs `cargo install
   uniffi-bindgen-python` (or equivalent). Easy to miss in onboarding docs.

In practice every meaningful UniFFI project ends up shipping its own thin
`uniffi-bindgen` binary — `hyperswitch-prism` itself does
(`-p uniffi-bindgen` in the workspace).

**proffi's design avoids all of this** by depending on UniFFI's bindgen
**libraries** (`uniffi_bindgen`, `uniffi_bindgen::bindings::python`,
`uniffi_bindgen::bindings::kotlin`) directly inside `proffi-cli`:

```rust
match lang {
    Lang::Python => uniffi_bindgen::bindings::python::generate(&lib_path, &crate_name, &out_dir, ...),
    Lang::Kotlin => uniffi_bindgen::bindings::kotlin::generate(...),
    Lang::JavaScript => self.render_koffi_template(&out_dir, &manifest, &descriptor),
}
```

Consequences:

- **No external bindgen binaries on PATH.**
- **Single source of UniFFI version truth.** `proffi-cli`'s `Cargo.lock` pins
  UniFFI; the user-facing `proffi` crate re-exports the same pinned `uniffi`.
  Adding `proffi = "0.1"` gets a coherent set automatically.
- **Crate name resolved from the manifest.** Stored at build time, passed to
  the bindgen call.

The only external tool that remains required on `PATH` is `protoc`, used for
generating user-language proto stubs (`*_pb2.py`, Kotlin proto, ts-proto).
Vendoring `protoc` is out of scope for MVP.

### 7.5 Generated-file headers

Every generated file gets a header:

```
// AUTO-GENERATED by proffi 0.1.0 from manifest.json
// rpc: greeter.Greeter.SayHello
// DO NOT EDIT — regenerate with `proffi generate --lang <x>`
```

Mirrors prism's convention; makes drift visible in code review.

## 8. Testing strategy

| Crate | Test type | What it covers |
|---|---|---|
| `proffi-runtime` | Unit tests | Runner success, decode failure, panic catch, domain error path. |
| `proffi-macros` | `trybuild` tests | Valid handler, wrong return type, missing `service=`, async handler (must fail with clear diagnostic in MVP). |
| `proffi-build` | Integration tests | Fixture crates with: (a) handler pointing to non-existent rpc, (b) rpc without handler, (c) type mismatch. Each must fail build with the documented diagnostic. |
| `proffi-cli` | Golden-file tests | Canned manifest+descriptor inputs, snapshot generated wrappers, diff on regen. |
| `examples/greeter` | End-to-end CI | Build cdylib, run `proffi generate` for all three languages, run a per-language smoke test that calls `say_hello` and asserts the result. |

The greeter end-to-end test is the load-bearing integration test: it would
catch any cross-cutting regression including UniFFI version drift, koffi ABI
breakage, or template/manifest schema mismatch.

## 9. Risks and open questions

### 9.1 Risks

1. **UniFFI version pinning.** `proffi` re-exports `uniffi` and `proffi-cli`
   links the matching `uniffi_bindgen`. If a user's other dependencies pull a
   conflicting `uniffi` version, Cargo's resolver surfaces it but the error may
   be cryptic. *Mitigation:* document in README; consider `proffi check-versions`
   subcommand later.
2. **prost-generated type paths in the manifest.** The macro records the Rust
   path of `Req`/`Res` (e.g. `proto::HelloRequest`) and `proffi-build` validates
   it resolves to the proto's `greeter.HelloRequest`. The mapping between proto
   package and Rust module is `prost-build`'s output, which is configurable
   (`module_attribute`, etc.). We need to read prost's generated module map
   alongside the descriptor. Feasible — prost emits a `.rs` per package that
   we can hash-match — but fiddly.
3. **JS/koffi `RustBuffer` ABI stability.** UniFFI's `RustBuffer` struct
   layout has been stable but is technically internal. If UniFFI changes it,
   the hand-rolled koffi binding breaks. *Mitigation:* pin UniFFI version
   (already done); CI re-runs the JS smoke test on every UniFFI bump; document
   the pinning expectation prominently.

### 9.2 Open questions deferred to implementation

- **Template engine choice.** `tera` (Jinja2-like, mature) vs `minijinja`
  (smaller, no_std-friendly). Resolve during scaffolding; both are fine; pick
  whichever has cleaner inheritance for templates that share boilerplate.
- **`proffi.proto` distribution.** Embed via `prost-build` for MVP (simplest);
  publish as a public proto package for users to import directly later if
  there's demand.

## 10. Success criteria

The MVP is done when:

1. `cargo install proffi-cli` and adding `proffi = "0.1"` to a fresh crate is
   sufficient to author and build a multi-rpc cdylib.
2. `proffi generate --lang python|kotlin|javascript --out <dir>` produces a
   working SDK that loads the cdylib, calls each rpc with a typed request, and
   returns a typed response.
3. The `examples/greeter` CI passes end-to-end across macOS aarch64 and
   Linux x86_64.
4. A handler that mismatches its proto rpc fails the build with a clear
   diagnostic pointing to the offending source line.
5. A panicking handler is caught and surfaced as `proffi.Error{code="HANDLER_PANIC"}`
   in every target language.
6. The runtime exposed to users is small enough — the `proffi-runtime` crate
   should be readable in a single sitting.

## 11. Future work (post-MVP)

- Async handlers behind `#[proffi::rpc(async)]`.
- Streaming RPCs.
- Swift, Ruby, Go, C# targets.
- Cross-compilation orchestration helpers (cargo-zigbuild integration,
  universal-binary recipes).
- Packaging templates: Python wheel (`maturin`-style), npm tarball, gradle
  module, podspec.
- A `proffi init` scaffolder that creates a working cdylib crate from a
  `.proto` skeleton.
- `proffi.proto` published as a standalone proto package.
