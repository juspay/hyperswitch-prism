# mock

A deterministic HTTP backend for the UCS Dummy connector. Used by Euler's
automation to validate proto / payment-method / flow coverage without hitting
any real PSP sandbox.

## Architecture

```
grpcurl
   │
   ▼  to :8000 (UCS grpc-server, "main grpc server")
grpc-server (hyperswitch-prism)
   │ x-connector: dummy
   ▼
Dummy Rust connector (crates/integrations/.../connectors/dummy.rs)
   │ HTTP POST (Stripe-shape, form-urlencoded, Bearer-auth)
   ▼
mock :8777 (axum)
   │
   ▼  handlers/<flow>.rs (thin: parse + auth + dispatch)
   │
   ▼  services/dummy/<flow>.rs (scenario logic + state mutation)
   │
   ▼  Stripe-shape JSON response ←─ flows back up the chain
```

Two processes, always. mock is HTTP-only. The internal `handlers/` →
`services/dummy/` split keeps protocol parsing separated from business logic
so future connector mocks can plug into the same scenario layer without
duplicating Stripe-form-parsing code.

## Run it (two terminals)

```bash
# Terminal A — mock HTTP backend on 8777
cargo run -p mock --release

# Terminal B — UCS grpc-server on 8000, forwards x-connector: dummy traffic to mock
cargo run -p grpc-server
```

The workspace configs (`config/{development,sandbox,production}.toml`,
`config/superposition.toml`) already point `dummy.base_url` at
`http://localhost:8777/dummy/`, so no env override is needed on grpc-server.

Then point grpcurl at grpc-server:

```bash
bash mock/grpc-test-commands.sh
```

## Direct HTTP testing (no grpc-server)

mock is also a self-contained Stripe-shape HTTP server. You can curl it
directly without standing up grpc-server — useful for testing the mock's
scenarios in isolation:

```bash
curl -H 'Authorization: Bearer sk_test_dummy' \
  http://localhost:8777/dummy/v1/payment_intents \
  -d 'amount=1000&currency=usd&payment_method_data[type]=card&payment_method_data[card][number]=4242424242424242'
# {"id":"pi_...", "status":"succeeded", ...}

curl -H 'Authorization: Bearer sk_test_dummy' \
  http://localhost:8777/dummy/v1/payment_intents \
  -d 'amount=1000&currency=inr&payment_method_data[type]=upi&payment_method_data[upi][vpa_id]=success@upi'
# {"id":"pi_...", "status":"succeeded", ...}
```

## Env vars

| Variable | Default | Notes |
|---|---|---|
| `DUMMY_BACKEND_BIND` | `127.0.0.1:8777` | Address the Axum server binds to. |
| `MOCK_DUMMY_PUBLIC_URL` | `http://localhost:8777` | Public host:port baked into `next_action.redirect_to_url`. Keep aligned with `DUMMY_BACKEND_BIND`. |
| `MOCK_DUMMY_REQUIRE_TOKEN` | unset | Optional shared-secret Bearer token. When set, mock requires this exact token; when unset, any non-empty non-`sk_live_*` token is accepted (Stripe test-mode style). |
| `RUST_LOG` | `info,mock=debug` | Standard `tracing-subscriber` filter. |

## Auth

`Authorization: Bearer <token>` is required on every API endpoint except
`GET /dummy/redirect/:attempt_id` (browser-facing). mock returns `401` when:

- the `Authorization` header is missing, or the Bearer value is empty;
- the token starts with `sk_live_` (case-insensitive — production keys are
  rejected so misconfigured smoke tests fail loudly);
- `MOCK_DUMMY_REQUIRE_TOKEN` is set and the supplied token does not match.

## Internal layout

```
mock/src/
├── main.rs              # tokio main; axum bind on 8777
├── router.rs            # 12 Stripe-shape routes + redirect + admin
├── auth.rs              # Bearer middleware
├── form.rs              # serde_qs non-strict extractor (accepts %5B/%5D)
├── state.rs             # AppState — 3 DashMaps + require_token
├── scenarios.rs         # classify_card / classify_upi / is_redirect_pm
├── types.rs             # Stripe-shape response types (PaymentIntent, Charge, Refund, …)
├── handlers/            # HTTP layer — protocol translation only
│   ├── authorize.rs     # AuthorizeReq struct + thin axum handler
│   ├── sync.rs          # GET /v1/payment_intents/:id
│   ├── capture.rs       # CaptureReq + POST .../capture
│   ├── void.rs          # POST .../cancel
│   ├── refund.rs        # RefundReq + POST /v1/refunds
│   ├── refund_sync.rs   # GET /v1/refunds/:id
│   ├── redirect.rs      # browser-facing GET /dummy/redirect/:attempt_id
│   ├── webhook_trigger.rs   # admin POST /dummy/admin/trigger-webhook
│   ├── common.rs        # load_intent / resource_missing / intent_not_found
│   └── mod.rs
└── services/
    ├── mod.rs           # pub mod dummy;
    └── dummy/           # business logic — protocol-agnostic
        ├── authorize.rs # scenario dispatch + state insert
        ├── sync.rs      # load_intent passthrough
        ├── capture.rs   # state guard + status flip
        ├── void.rs      # state guard + cancel
        ├── refund.rs    # state guard + refund record creation
        ├── refund_sync.rs   # refunds map lookup
        └── mod.rs
```

The handler files import request structs (e.g. `AuthorizeReq` from
`handlers/authorize.rs`), parse incoming form bodies via `StripeForm<T>`, and
delegate to `services::dummy::<flow>::<fn>`. The service functions take those
request structs by reference, mutate `AppState`, and return Stripe-shape
response types (or `Result<T, axum::Response>` for typed 4xx errors).

This split means:
- **Handlers** only know HTTP + Stripe-shape parsing.
- **Services** only know scenarios + state mutation, no protocol knowledge.
- Adding a different protocol entry point later (gRPC, kafka, …) would mean
  adding a new handler module that hits the same services. No business-logic
  duplication.

## Scenarios

### Cards — keyed off `payment_method.card.card_number`

| PAN | Outcome |
|---|---|
| `4242424242424242`, `4111111111111111`, `5555555555554444`, `378282246310005`, `5200828282828210` | `succeeded` (or `requires_capture` if `capture_method=manual`) |
| `4000003800000446` | `requires_action` (3DS) — succeeds after redirect visit |
| `4000000000000002` | `failed` with `card_declined` / "Your card was declined." |
| anything else | `failed` with `card_not_supported` |

### UPI — keyed off `payment_method.upi.vpa_id`

| VPA | Outcome |
|---|---|
| `success@upi` | `succeeded` |
| `failure@upi` | `failed` (`upi_declined`) |
| anything else | `failed` (`invalid_vpa`) |

### Redirect payment methods

`bancontact`, `ideal`, `trustly`, `blik`, `mb_way`, `satispay`, `wero`,
`alipay`, `wechat_pay`, `revolut_pay` all return `requires_action` plus a
redirect URL. Visiting the URL completes the attempt:

- plain GET → `succeeded`
- `?reject=1` → `failed` (`redirect_rejected`)
- `?manual=1` → renders a human-readable HTML page; state still updates.

## Status mapping (Stripe → Hyperswitch)

| Stripe (returned by mock) | Hyperswitch attempt status |
|---|---|
| `succeeded` | `Charged` |
| `requires_capture` | `Authorized` |
| `requires_action` | `RequiresCustomerAction` |
| `canceled` | `Voided` |
| `failed` | `Failure` |

Refund flows always emit `succeeded` (no refund-failure scenarios in scope).

## Admin webhook trigger

`POST /dummy/admin/trigger-webhook` (Bearer-auth required) fires a Stripe-shape
event to an arbitrary URL. Useful for driving downstream webhook ingestion code
without waiting on a real PSP callback.

```bash
curl -H 'Authorization: Bearer sk_test_dummy' -H 'Content-Type: application/json' \
  http://localhost:8777/dummy/admin/trigger-webhook \
  -d '{
    "target_url": "http://localhost:9000/webhook-sink",
    "payment_intent_id": "pi_abc123",
    "event_type": "payment_intent.succeeded"
  }'
```

Returns `{delivered_to, status, event_id}`. The outbound `reqwest` client has a
10-second timeout.

## Internals

- State lives in `AppState` — three `DashMap`s for payment intents, refunds,
  and pending redirect attempts. No persistence across restarts (intentional).
- A background TTL sweeper runs every 5 minutes and evicts entries older than
  60 minutes.
- HTML interpolation in the redirect page html-escapes inputs defensively.
- Form parser is `serde_qs::Config::new(10, false)` — **non-strict** mode.
  **Do not switch back to strict.** The Dummy Rust connector URL-encodes form
  keys per RFC 3986 (brackets become `%5B`/`%5D`), and strict mode would
  silently misparse every real request as `payment_method_not_supported`.

## Out of scope

- BNPL (Klarna, Afterpay, Affirm).
- Full-fidelity SetupMandate.
- Full-fidelity IncrementalAuthorization (echo stub only).
- 3DS challenge UI beyond the single 3DS test card.
- Redis-backed storage.
- Persistent fixtures across restarts.

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| Integration tests fail with `ConnectorNotFound` | `creds.json` is missing the `dummy` block. Add `{"dummy": {"api_key": {"value": "sk_test_dummy"}}}`. |
| Every form request rejected as `payment_method_not_supported` | Someone reverted the non-strict `serde_qs` change — see "Internals". |
| Redirect URLs point to `localhost:8000` but mock listens on `8777` | `MOCK_DUMMY_PUBLIC_URL` is unset or stale. Set to externally-reachable host:port. |
| `Address already in use` on bind | Port conflict. Pick a different `DUMMY_BACKEND_BIND` port. |
| `401 Unauthorized` on every request | Missing `Authorization: Bearer <token>`, token starts with `sk_live_`, or `MOCK_DUMMY_REQUIRE_TOKEN` mismatch. |
| grpcurl returns `Invalid connector: Matching variant not found` | grpc-server's ConnectorEnum doesn't include `Dummy`. Either you're on a stale grpc-server binary or the proto wasn't regenerated — `cargo clean -p grpc-api-types && cargo build -p grpc-server`. |
| grpcurl reflection shows `JUSPAY = 122` instead of `DUMMY = 122` | A different gRPC server (typically a VS Code Node helper) is bound to `127.0.0.1:8000` and intercepting your call. Kill it: `pkill -f 'Code Helper (Plugin)'`. |
