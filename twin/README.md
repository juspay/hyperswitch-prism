# twin

A deterministic HTTP backend that the UCS Dummy connector talks to. Used by Euler's automation to validate proto / payment-method / flow coverage without hitting any real PSP sandbox.

"Twin" — short for *digital twin of a PSP*. Stateful, production-parity, deterministic. Not a stub, not a fixture server.

## Why it exists

No real PSP sandbox is a superset of Euler⇄UCS payment methods (Stripe lacks RevolutPay-wallet, Revolut lacks cards, PPRO is redirect-only, UPI has no public sandbox). Twin covers the full matrix in one place, no network deps, deterministic outcomes keyed off input.

## Architecture (two-process)

```
grpcurl
   │  gRPC :8000
   ▼
grpc-server (hyperswitch-prism)
   │  resolves x-connector-config → "dummy"
   ▼
Dummy Rust connector (real production code path)
   │  HTTP POST application/x-www-form-urlencoded
   │  Authorization: Bearer <api_key>
   ▼
twin :8777
   │  router.rs → /dummy/v1/*
   ▼  auth.rs → Bearer
   ▼  handlers/<flow>.rs   protocol layer (form parse → dispatch)
   ▼  services/dummy/<flow>.rs   business logic + state mutation
   ▼  state.rs   DashMap<id, PaymentIntent>
   ▲  types.rs   Stripe-shape JSON
   ▲  serialized 200 OK
```

Two processes always. Twin is HTTP-only on 8777. Grpc-server fronts on 8000.

## Run locally

Three terminals.

```bash
# Terminal A — twin
cargo run -p twin --release
# logs: "twin listening" on 127.0.0.1:8777

# Terminal B — grpc-server
cargo run -p grpc-server
# logs: "starting connector service" on 0.0.0.0:8000

# Terminal C — fire grpcurl (see below)
```

## Hello world

```bash
grpcurl -plaintext \
  -H 'x-connector-config: {"config":{"Dummy":{"api_key":"sk_test_dummy","base_url":"http://localhost:8777/dummy/"}}}' \
  -H 'x-merchant-id: m_test' -H 'x-tenant-id: default' -H 'x-request-id: r1' \
  -H 'x-connector-request-reference-id: ref1' \
  -d '{
  "merchant_transaction_id":"t1","amount":{"minor_amount":1000,"currency":"USD"},
  "payment_method":{"card":{
    "card_number":{"value":"4242424242424242"},
    "card_exp_month":{"value":"03"},"card_exp_year":{"value":"2030"},
    "card_cvc":{"value":"737"},"card_holder_name":{"value":"John Doe"}}},
  "capture_method":"AUTOMATIC","address":{"billing_address":{}},
  "auth_type":"NO_THREE_DS","return_url":"https://example.com/return"
}' localhost:8000 types.PaymentService/Authorize
```

Expected: `"status": "CHARGED"`.

Full 26-cell matrix lives in [`grpc-curls.txt`](grpc-curls.txt) (raw copy-paste) and [`grpc-test-commands.sh`](grpc-test-commands.sh) (chained with jq).

## Scenarios

**Cards** (`payment_method_data[card][number]`):

| PAN | Outcome |
|---|---|
| `4242424242424242` & 4 other known PANs | `succeeded` (`requires_capture` if manual) |
| `4000003800000446` | `requires_action` (3DS), succeeds on redirect complete |
| `4000000000000002` | `failed` with message containing "declined" |
| anything else | `failed` (`card_not_supported`) |

**UPI** (`payment_method_data[upi][vpa_id]`):

| VPA | Outcome |
|---|---|
| `success@upi` | `succeeded` |
| `failure@upi` | `failed` (`upi_declined`) |

**Redirect PMs** (`bancontact`, `ideal`, `trustly`, `blik`, `mb_way`, `satispay`, `wero`, `alipay`, `wechat_pay`, `revolut_pay`): all return `requires_action` + redirect URL. Plain GET on the URL completes to `succeeded`; appending `?reject=1` flips to `failed` (`redirect_rejected`).

## Run in Docker

```bash
cd twin
docker compose up
```

Builds the image, exposes :8777, runs the matrix against `localhost:8000` (grpc-server still on the host).

## Env vars

| Var | Default | Purpose |
|---|---|---|
| `TWIN_BIND` | `127.0.0.1:8777` | Address the axum server binds to. |
| `TWIN_PUBLIC_URL` | `http://localhost:8777` | Public host:port baked into `next_action.redirect_to_url`. Keep aligned with `TWIN_BIND`. |
| `TWIN_REQUIRE_TOKEN` | unset | Optional shared-secret Bearer token. When set, twin requires this exact token; when unset, any non-empty non-`sk_live_*` token is accepted (Stripe test-mode style). |
| `RUST_LOG` | `info,twin=debug` | Standard `tracing_subscriber` filter. |

## Auth model

Every API endpoint under `/dummy/v1/*` and `/dummy/admin/*` requires `Authorization: Bearer <token>`. The redirect page `/dummy/redirect/:attempt_id` is public (browsers don't send Bearer).

Twin rejects:
- empty / missing token
- any token whose prefix is `sk_live_` (case-insensitive) — guards against accidentally pointing production keys at twin
- mismatches against `TWIN_REQUIRE_TOKEN` if it's set

## Status mapping (Stripe → Hyperswitch)

The Dummy Rust connector handles this — listed here for reference:

| Twin / Stripe | gRPC `PaymentStatus` |
|---|---|
| `succeeded` | `CHARGED` |
| `requires_capture` | `AUTHORIZED` |
| `requires_action` | `AUTHENTICATION_PENDING` |
| `canceled` | `VOIDED` |
| `failed` | `FAILURE` |

## File map

| File | Role |
|---|---|
| `src/main.rs` | Bootstrap: reads `TWIN_BIND`, starts axum, spawns TTL sweeper. |
| `src/router.rs` | Routes: 12 Stripe-shape paths under `/dummy/v1/*` + redirect page + admin webhook trigger. |
| `src/auth.rs` | Bearer middleware. |
| `src/form.rs` | `StripeForm<T>` extractor. `serde_qs::Config::new(10, false)` — **non-strict** so URL-encoded `%5B`/`%5D` brackets parse the same as raw brackets. Don't revert. |
| `src/state.rs` | Three `Arc<DashMap<...>>`s (`payment_intents`, `refunds`, `attempts`) + 5-min sweeper, 60-min TTL. |
| `src/scenarios.rs` | `classify_card`, `classify_upi`, `is_redirect_pm`. |
| `src/types.rs` | Stripe-shape JSON: `PaymentIntent`, `Charge`, `NextAction`, `RedirectToUrl`, `Refund`, `StripeErrorObject`, `IntentStatus`. |
| `src/handlers/*.rs` | Per-flow protocol layer: parse form body, dispatch, serialize JSON. |
| `src/services/dummy/*.rs` | Per-flow business logic + state mutation. Inputs are Stripe-shape request structs; outputs are Stripe-shape `PaymentIntent` / `Refund` (or `Err(Response)` for 4xx). |

## Troubleshooting

| Symptom | Likely cause |
|---|---|
| `connection refused` on :8777 | Twin not running, or bound to a different port. |
| Every Authorize returns `payment_method_not_supported` | Twin's `form.rs` is in strict mode. Should be `serde_qs::Config::new(10, false)`. |
| Redirect URLs point to `localhost:8000` but twin listens on `8777` | `TWIN_PUBLIC_URL` is unset or stale. Set to externally-reachable host:port. |
| `Address already in use` on bind | Port conflict. Pick a different `TWIN_BIND` port. |
| `401 Unauthorized` on every request | Missing `Authorization: Bearer <token>`, token starts with `sk_live_`, or `TWIN_REQUIRE_TOKEN` mismatch. |

## Where this is going

Twin is single-connector today (Dummy). Roadmap is a multi-connector platform with a `ConnectorTwin` trait, shared infra (idempotency, failure injection, async webhooks, contract tests), and one `connectors/<name>/` directory per PSP.
