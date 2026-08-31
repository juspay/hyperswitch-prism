# Implementation plan: déjà instrumentation in hyperswitch-prism

Scope: the **prism-side, record-capable** work — Stages A + B of the [proposal](./deja-ucs-proposal.md). This is everything needed to *record*; the déjà-library changes and the replay driver (Stage C) are out of scope here and tracked separately. Technical detail for each item is in the [RFC](./deja-ucs-integration.md); this doc is the execution order and checklists.

**Prime invariant for every PR:** normal working is never affected. Each PR must compile **feature-off byte-identical** and **feature-on with no hook = pure passthrough**. Nothing records until PR-5 installs the boot hook.

---

## Before you start — lock these decisions

- [ ] **Pin the déjà rev.** Choose the `rev` for the git dep; align with hyperswitch `main`'s pin. (RFC §6.1)
- [ ] **Pin `superposition_core`.** It currently tracks `branch = "main"` in the workspace — a build-stability risk independent of déjà. Pin a rev in the same PR-1. (RFC §12.3)
- [ ] **Confirm sandbox-only posture** and injector opt-in/digest handling with the architect. (Proposal §6.1)
- [ ] **Agree the envelope is frozen at v2** before PR-5 emits tapes. (RFC §6.5)
- [ ] Read RFC §4 (the invariant matrix) and §11 (failure-policy matrix) — every PR is reviewed against them.

## PR sequence at a glance

| PR | Title | Touches request path? | Records? | Depends on |
|---|---|---|---|---|
| **PR-1** | Foundation: features, config, purity proof | no | no | — |
| **PR-2** | Shared entropy seams + lint | seams only | no | PR-1 |
| **PR-3** | Correlation + gRPC ingress boundary | yes | no (inert) | PR-1 |
| **PR-4** | Egress boundaries ×3 | yes | no (inert) | PR-1 |
| **PR-5** | Sink + boot + sampler + release — **recording goes live** | yes | **yes (sandbox)** | PR-1, PR-3, PR-4 |
| **PR-6** | Connector entropy migration (batched, ongoing) | seams only | — | PR-2 |

PR-2, PR-3, PR-4 are independent of each other (all inert) and can proceed in parallel after PR-1. PR-5 lands after 3+4 so there are boundaries to capture. PR-6 runs alongside from after PR-2.

---

## PR-1 — Foundation

**Goal:** the dependency, feature ladder, typed config, and the feature-off purity proof. Zero runtime behavior.

**Files**
- `Cargo.toml` (workspace) — `deja` in `[workspace.dependencies]` (rev-pinned); pin `superposition_core` rev.
- Per-crate `Cargo.toml` feature lines: `common_utils` (`dep:deja`), `domain_types`, `interfaces` (add `[features]` if absent), `connector-integration` (`dep:deja`), `external-services` (`dep:deja`), `ucs_env` (`deja = []`, dep-free), `ucs_interface_common`, `grpc-server` (umbrella + `dep:deja`, `dep:deja-core`).
- `crates/common/ucs_env/src/deja_config.rs` **(new)** — `DejaConfig` and sub-structs (all `#[cfg(feature="deja")]`, `#[serde(default)]`), `DejaMode` enum, `validate()`.
- `crates/common/ucs_env/src/lib.rs` — `#[cfg(feature="deja")] pub mod deja_config;`
- `crates/common/ucs_env/src/configs.rs` — add `#[cfg(feature="deja")] #[serde(default)] #[patch(ignore)] pub deja: DejaConfig`; cfg-gated `with_list_parse_key("deja.recording.kafka.brokers")`; cfg-gated `validate()` call in the loader.
- `config/{development,sandbox,production}.toml` — inert `[deja]` blocks (mode = "disabled").
- `crates/common/ucs_env/tests/deja_config_purity.rs` **(new)**.

**Tasks**
- [ ] Add workspace dep + superposition rev pin; regenerate `Cargo.lock`.
- [ ] Wire the feature ladder; verify `cargo tree -p grpc-server` (default) shows **zero** deja crates.
- [ ] Implement `DejaConfig` mirroring hyperswitch's `DejaSettings` shape (RFC §6.1) — note: **no `sampler.timeout_ms`** (sampler is synchronous, §PR-5).
- [ ] `DejaConfig::validate()` rejects `(Replay, Env::Production)` and `(Record, no brokers + no inheritance)`.
- [ ] Confirm the `#[patch(ignore)]` exclusion and the `deny_unknown_fields` rejection of `"deja"` in an override.
- [ ] Write the purity test with a canonicalizer (recursive key-sort + scalar-array sort — `Config` has HashMap/HashSet fields and serde_json `preserve_order` is on).

**Acceptance**
- [ ] `cargo build` (default) and `cargo build --features deja` both succeed.
- [ ] `cargo tree -p grpc-server` default = zero deja crates.
- [ ] Purity test passes feature-off (stripped vs injected `[deja]` → byte-identical `Config`).
- [ ] ConfigPatch-rejection test: an `x-config-override` containing `"deja"` is **rejected**, not merged.

**Invariant proof:** every change is `#[cfg]`-gated or an optional-dep/feature-table line; default token stream is unchanged.

---

## PR-2 — Shared entropy seams + lint

**Goal:** seam the shared time/id/crypto helpers and the request-id fallback; add the enforcement lint (as `warn`). No structural request-path change.

**Files**
- `crates/common/common_utils/src/lib.rs` — `deja::time` on `date_time::now`, `now_unix_timestamp`; add `now_unix_timestamp_nanos() -> i128` (byte codec).
- `crates/common/common_utils/src/fp_utils.rs` — `deja::id` on `generate_id*`, `generate_uuid_v7`; add `generate_uuid_v4()`.
- `crates/common/common_utils/src/crypto.rs` — `deja::id` on random string/bytes; extract `generate_gcm_nonce_bytes() -> [u8;12]` and seam the byte-fill (not the u128 — the nonce serialization lesson).
- `crates/common/common_utils/src/crypto/jose.rs` — route claim-time through the seamed `now_unix_timestamp()`.
- `crates/types-traits/domain_types/src/utils.rs` — `deja::id` on `generate_random_bytes`.
- `crates/types-traits/ucs_interface_common/src/metadata.rs` (~266) — request-id seam: record generated fallback; on replay, fail loud (recorded requests always carry the id).
- `clippy.toml` **(new, workspace root)** — `disallowed-methods`/`disallowed-types` for `now_utc`, `SystemTime::now`, `Uuid::new_v4/now_v7`, `thread_rng`, `OsRng`, `SystemRandom::new`; set the lints to `warn` in `[workspace.lints.clippy]`.

**Tasks**
- [ ] Add each seam with `#[cfg_attr(feature="deja", deja::time/id)]` + `#[cfg_attr(feature="deja", track_caller)]`.
- [ ] Confirm déjà's macro accepts a const-generic fn for `generate_cryptographically_secure_random_bytes::<N>`; else add a non-generic `fill_random_bytes` inner seam.
- [ ] Use **byte codecs** for i128 nanos and the nonce (serde_json::Number silently nulls > u64).
- [ ] Add `#[allow(clippy::disallowed_methods)] // deja seam interior` on the ~8 seam interiors + the 2 RSA-PKCS1 signing RNGs.

**Acceptance**
- [ ] Both builds pass; `clippy` clean at `warn`.
- [ ] Feature-on-no-hook: each seamed helper returns exactly as before (unit test).
- [ ] Round-trip test: recorded value substitutes on replay (in-process `LookupTableHook`).

**Invariant proof:** `cfg_attr` macros compile away feature-off; feature-on the generator runs unchanged (~1–2 ns).

---

## PR-3 — Correlation substrate + gRPC ingress boundary

**Goal:** capture the incoming request/response as one decoded event; wire the correlation layer and the sampler seam (with a default no-op sampler — the Superposition impl lands in PR-5).

**Files**
- `crates/grpc-server/grpc-server/src/deja/{mod,ingress,decode,sampler}.rs` **(new)** — all `#[cfg(feature="deja")]`.
- `crates/grpc-server/grpc-server/src/lib.rs` — `#[cfg(feature="deja")] pub mod deja;`
- `crates/grpc-server/grpc-server/src/app.rs` (~281) — splice the ingress layer **after `PropagateRequestIdLayer`, before `RequestExtensionsLayer`** (two cfg arms around the `.layer()` chain so feature-off is byte-identical).
- `crates/grpc-server/grpc-server/src/utils.rs` — sampler push in `grpc_logging_wrapper` + `_with_parser` (~335); `record_fields_from_header` (~32) span-field alignment.
- `crates/common/ucs_env/src/logger/setup.rs` — install `DejaCorrelationLayer` only when `process_mode().is_observing()`.
- `crates/grpc-server/grpc-server/Cargo.toml` — add `prost-reflect` (features `serde`).

**Tasks**
- [ ] Ingress `Layer`/`Service` over `http::Request<tonic::body::Body>` (template: `config_overrides.rs`); **inactive arm = pure passthrough**, no buffering.
- [ ] Request buffering (size cap ~4 MiB; over-cap → passthrough + lossy-marked event); response `CaptureBody` wrapper finalizing on trailers/EOS/Drop.
- [ ] Proto decode via `grpc_api_types::FILE_DESCRIPTOR_SET` (already emitted — no build change) → `prost_reflect` pool → proto3-JSON; decode failure → base64 fallback.
- [ ] Structural exclusions: `/grpc.health.v1.Health/*`, `/grpc.reflection.*`, streaming (none exist today).
- [ ] Sampler trait + `SamplerFacts` + RAII `DecisionGuard`; push gated on **boot-time `process_mode()==Record`**, never per-correlation `mode()`.
- [ ] `Span::current().record("request_id", …)` in the active arm so the correlation layer's `on_record` fires even when the client omits the header.

**Acceptance**
- [ ] Passthrough parity test: feature-on-no-hook, probe body that panics on unexpected buffering; response bytes/trailers/header order identical to the layerless stack.
- [ ] E2E (test hook, mode=Record): N unary calls → exactly N decoded events with correct rpc path, grpc-status, request_id; health + reflection → zero events.
- [ ] Process-mode invariant test: sampler `should_record` called zero times in Replay/Off.
- [ ] Guard-cleanup test: panic after push → decision cleared.

**Invariant proof:** inactive arm returns `inner.call(req)` untouched; layer splice is cfg-gated; existing layer order unchanged.

---

## PR-4 — Egress boundaries ×3

**Goal:** record/replay the connector HTTP call, the Kafka-transport call, and the injector call. Always-substitute (both Ok and Err arms).

**Files**
- `crates/common/external-services/src/service.rs` — boundary on `call_connector_api` (~1152); thin wrappers for the Kafka call site (~974) and `injector_core` (~802).
- `crates/common/external-services/src/deja_codec.rs` **(new)** — `TapeResponse`, `HttpTapeOutcome`, codec impls.
- `crates/common/common_enums/src/enums.rs` — cfg-gated `serde` derives on `ApiClientError`, `KafkaClientError`.
- `crates/common/external-services/Cargo.toml`, `crates/common/common_enums/Cargo.toml` — feature wiring.

**Tasks**
- [ ] HTTP: annotate `call_connector_api` directly (no Extensions smuggle — `handle_response` already materializes `Response`). Args = method, URL (post-rewrite), **headers sorted by (name,value)**, body by `RequestContent` variant (`RawBytes`→base64, `FormData`→**structured `MultipartData`**, not `render_as_bytes`); mTLS `certificate*` never captured; whole args `Secret`-wrapped.
- [ ] Codec over `CustomResult<Result<Response,Response>, ApiClientError>` — both inner error arm and outer error round-trip; add a comment at the two error mappers (attachment loss is verified safe).
- [ ] Kafka: wrap the single call site so `deja` composes with `connector-request-kafka` on/off; record the classified synthetic `Response`; replay publishes nothing (and skip `init_kafka_producer` in replay).
- [ ] Injector: wrap `injector_core`; args = endpoint + method + **SHA-256 digests** of template/headers/token ids; `sensitivity: vault` tag; opt-in even in sandbox.

**Acceptance**
- [ ] Codec round-trip property tests: BOM, non-UTF8 body, duplicate headers, 204/302/4xx/5xx, every error variant.
- [ ] Passthrough parity: feature on/off return byte-identical `Response` (cross-build digest compare).
- [ ] Record→replay with mock **stopped** + zero-new-requests assertion, for HTTP (Ok, 503, conn-refused), Kafka (queued/rejected/unknown), injector.
- [ ] Injector redaction test: recorded args contain no template/header plaintext.

**Invariant proof:** boundary sits below the audit-event emission and TestConfig rewrite (unaffected); inactive arm awaits the original body; feature-off erases the attribute.

---

## PR-5 — Sink + boot + sampler + release *(recording goes live)*

**Goal:** the dedicated Kafka sink, the boot install, the Superposition sampler, and the isolated release-enable. This is the Stage B milestone.

**Files**
- `crates/grpc-server/grpc-server/src/deja/{boot,record_sink}.rs` **(new)**; complete `sampler.rs` with the Superposition impl.
- `crates/grpc-server/grpc-server/src/main.rs` — `install()` **between the version stamp and `logger::setup`**.
- `crates/common/common_utils/src/superposition_config.rs` — add `resolve_flag(key, dims)`.
- `config/superposition.toml` — `deja_record=false` default, `rpc_method` dimension, sample-in overrides.
- `crates/grpc-server/grpc-server/Cargo.toml` — `dep:rdkafka` (optional), `dep:deja-core`.
- `Dockerfile` — append `,deja` to both `--features` lines **(separate final commit)**.
- `crates/grpc-server/grpc-server/tests/deja_boot_fail_loud.rs` **(new)**.

**Tasks**
- [ ] `deja_boot::install(&config.deja, Some(&config.events.brokers))`: Disabled → disabled hook; Record misconfig → **fail open** (disabled hook + `eprintln!`); Replay misconfig → **fail loud** (abort boot). Identity: run_id / instance_id (→ `runtime_metadata.pod_name`) / code_sha (→ `VERGEN_GIT_SHA`). Boot.rs uses raw `SystemTime`/`pid` (never seams).
- [ ] `UcsKafkaRecordSink`: dedicated `ThreadedProducer` (no constructor metadata probe), hardened (`acks=all`, idempotence, bounded buffers); three envelopes (`deja_artifact_record` v2 / `deja_graph_node` v1 / `deja_sink_marker` v2) — **field-for-field the hyperswitch contract**; partition key = correlation id; cadence flush 50 ms, EOF marker 10 s drain.
- [ ] Sampler: **synchronous** in-process `eval_config` (drop `timeout_ms`); structural exclusion of non-`/ucs.` paths; memoize per rpc method; eval error → `!fail_closed`.
- [ ] Add an explicit `flush_global_runtime_hook()` on SIGTERM after `try_join!` (writer-drop EOF alone is fragile).

**Acceptance**
- [ ] Fail-open tests (record, no topic / no brokers / bad broker) → `Ok(disabled)`.
- [ ] Fail-loud tests (replay, no source / missing file) → `Err`, boot aborts.
- [ ] Envelope-shape pin tests (v2 artifact / v1 graph / v2 marker) — the cross-repo contract.
- [ ] Sampler tests: default-false, override-true, health/reflection excluded, `fail_closed` both ways.
- [ ] Sandbox smoke: record a real flow → tape lands with decoded ingress + egress + seams.

**Invariant proof:** boot install is cfg-gated and precedes any hook peek; record path is fail-open; Dockerfile change is a standalone revertible commit; default local builds stay feature-off.

---

## PR-6 — Connector entropy migration *(batched, ongoing)*

**Goal:** migrate the ~61 direct entropy sites (32 connectors) to the seamed helpers from PR-2; flip the lint to `deny`. Reduces replay noise; un-migrated sites are safe meanwhile (scored divergences, never live calls).

**Batches (signed-request first)**
- [ ] **Batch A** — signature/HMAC entropy: paytm (SystemRandom salt), rapyd/globalpay/authorizedotnet (`thread_rng`), grabpay (RFC7231 date), fiserv (millis + uuid), cybersource family (cybersource/bankofamerica/barclaycard/wellsfargo/payout-cybersource), deutschebank CSEAL, paybox/qwikcilver.
- [ ] **Batch B** — uuid idempotency/client-request ids → `generate_uuid_v4()` (do **not** switch to v7).
- [ ] **Batch C** — payload timestamps (peachpayments, kount, twoc_twop_paco, adyen, pinelabs, getnet, plaid).
- [ ] Flip `clippy.toml` lints `warn` → `deny` once batches land.

**Acceptance**
- [ ] Lint enforced in CI.
- [ ] Top-traffic connectors self-replay with zero argument divergences.

**Do NOT seam** (exclude via `reply_canon` projection): audit-event timestamps (`service.rs` ~1115, `utils.rs` ~510), `Instant` latency tracking, `date_time::time_it`. **Open item:** josekit JWE CEK/IV is unseamable — decide projection vs accept before touching JWE-emitting flows.

---

## Definition of done (prism-side)

- [ ] PR-1…PR-5 merged; PR-6 in progress or scheduled.
- [ ] Default builds byte-identical; `--features deja` builds clean in CI (`clippy --all-features`, `--each-feature`).
- [ ] Sandbox records a representative workload; tapes are decoded, complete, and deterministic across two runs (modulo seams).
- [ ] Envelope v2 frozen; sink contract pinned by test.
- [ ] All boundary passthrough-parity + record→replay round-trip tests green.
- [ ] Optional de-risk (proposal §4): one correlation replayed offline via the D3 *fallback* + a throwaway driver, proving end-to-end tape sufficiency.

## Out of scope here (Stage C — tracked separately)

Déjà-library changes (D1 `deja-tonic`, D2 kernel ingress driver, D3 ingress-root recognition), the `crates/internal/replay-driver` crate, and the CI regression-gate workflows. These depend on the tapes this plan produces and are detailed in RFC §6.6 and §7.

## Open questions to resolve during implementation

1. Déjà macro surface: does it expose `substitute`/`codec` args, or is the thin shell-fn split needed? (affects PR-4 shape)
2. Const-generic seam support for `generate_cryptographically_secure_random_bytes::<N>` (PR-2 fallback ready).
3. Connector-cred masking vs egress lookup identity — validate during PR-4 review (RFC §12.4).
4. josekit JWE projection decision (PR-6).
5. HTTP service-mode ingress — explicitly deferred; revisit only if it serves recorded traffic.
