# Authentication Dispatch Pattern (`next_authentication_step`)

## Overview

The three standalone 3DS legs — [`PreAuthenticate`](./pattern_preauthenticate.md), [`Authenticate`](./pattern_authenticate.md), [`PostAuthenticate`](./pattern_postauthenticate.md) — are not self-scheduling. Implementing the three `ConnectorIntegrationV2` blocks makes them *callable*; it does not make them *called*. The composite authorize flow asks the connector, once per loop iteration, which leg to run next, through a single `ValidationTrait` hook: `next_authentication_step`. Its default returns `AuthenticationStep::Authorize`, which means "skip every authentication leg and charge the card".

A connector that implements the trio but does not override `next_authentication_step` has three unreachable flows on the composite path. This is the single most common way a correctly-written 3DS integration produces a non-3DS payment. The scaffold generator emits an **empty** `ValidationTrait` impl (`grace/rulesbook/codegen/add_connector.sh`, the `===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====` block), so every generated connector starts in exactly that state.

> **CROSS-REPO DEPENDENCY — READ THIS BEFORE DEBUGGING.** UCS is only half the state machine. The Hyperswitch **router** independently gates whether it continues past each leg, via a per-connector match in `crates/router/src/core/payments/flows/authorize_flow.rs` (`should_continue_after_preauthenticate` / `should_continue_after_authenticate`) whose default is `false`. A new connector's 3DS therefore stops silently after leg 1 even with a perfect UCS override, until that router-side match is extended. That file does **not** exist in this repository (`ls crates/router` → no such directory); it is a separate deliverable in the hyperswitch repo. The dependency is documented in-tree at `crates/integrations/connector-integration/src/connectors/saferpay/transformers.rs`, in the doc comment on `fn is_three_ds_settlement`: *"the caller stops there (`should_continue_after_preauthenticate` defaults to false)"*. See [The router-side gate](#the-router-side-gate).

### Key Components
- Hook: `ValidationTrait::next_authentication_step` in `crates/types-traits/interfaces/src/connector_types.rs` (`pub trait ValidationTrait`).
- Step enum: `pub enum AuthenticationStep` — same file, four variants.
- Redirect enum: `pub enum RedirectState` — same file, three variants.
- Driver: `fn process_composite_authorize` in `crates/internal/composite-service/src/payments.rs` — the **only** call site of the hook in the whole tree.
- Loop state: `struct AuthorizeCompositeState` — same file.
- Redirect classifier: `fn get_redirect_state` — same file.
- Break predicates: `pub fn is_failure_payment_status` / `pub fn is_terminal_payment_status` in `crates/internal/composite-service/src/utils.rs`.
- Transport: `rpc Authorize(CompositeAuthorizeRequest)` on `service CompositePaymentService` in `crates/types-traits/grpc-api-types/proto/composite_services.proto`.

## Table of Contents

1. [Overview](#overview)
2. [Architecture Overview](#architecture-overview)
3. [The composite loop, step by step](#the-composite-loop-step-by-step)
4. [Deriving RedirectState](#deriving-redirectstate)
5. [Connectors with Full Implementation](#connectors-with-full-implementation)
6. [Common Implementation Patterns](#common-implementation-patterns)
7. [Connector-Specific Patterns](#connector-specific-patterns)
8. [Code Examples](#code-examples)
9. [Integration Guidelines](#integration-guidelines)
10. [Decision table: choosing your arms](#decision-table-choosing-your-arms)
11. [The router-side gate](#the-router-side-gate)
12. [Best Practices](#best-practices)
13. [Common Errors / Gotchas](#common-errors--gotchas)
14. [Testing Notes](#testing-notes)
15. [Cross-References](#cross-references)

## Architecture Overview

### Flow Hierarchy

```
CompositePaymentService.Authorize  (composite_services.proto)
│
└── process_composite_authorize            (composite-service/src/payments.rs)
      ├── create_server_authentication_token / session token / customer / order
      ├── auth_type      := get_auth_type(&payload)         → common_enums::AuthenticationType
      ├── payment_method := get_payment_method(&payload)    → common_enums::PaymentMethod
      ├── redirect_state := get_redirect_state(&payload)    → RedirectState      ── computed ONCE
      ├── state          := AuthorizeCompositeState::default()   (completed_step = None)
      │
      └── loop {
            next_step = connector.next_authentication_step(
                            auth_type, payment_method, redirect_state, state.completed_step)
            match next_step {
              PreAuthenticate  → run leg, set completed_step, break if redirect|failure
              Authenticate     → run leg, set completed_step, break if redirect|terminal
              PostAuthenticate → run leg, set completed_step, NO BREAK
              Authorize        → run authorize, break
            }
          }
```

### The hook

Declared as a defaulted method on `ValidationTrait`, so every connector already satisfies it — silently.

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — pub trait ValidationTrait
    /// Returns the next authentication step for composite authorize flow.
    /// The connector examines the current state and returns which step should execute next.
    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: PaymentMethod,
        _redirect_state: RedirectState,
        _completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        AuthenticationStep::Authorize
    }
```

The default body is `AuthenticationStep::Authorize` — the loop's terminating arm. That is the whole reason this pattern file exists.

### The step enum

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — pub enum AuthenticationStep
/// Represents the next authentication step for composite authorize flow.
/// Connectors implement `next_authentication_step` to guide the flow controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthenticationStep {
    /// Run PreAuthenticate (typically device data collection setup)
    PreAuthenticate,
    /// Run Authenticate (typically challenge initiation)
    Authenticate,
    /// Run PostAuthenticate (typically challenge validation)
    PostAuthenticate,
    /// Stop authentication loop and proceed to Authorize
    Authorize,
}
```

### The redirect enum

```rust
// From crates/types-traits/interfaces/src/connector_types.rs — pub enum RedirectState
/// Represents the redirect state for composite authorize flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RedirectState {
    InitialRequest,
    RedirectWithParams,
    RedirectWithoutParams,
}
```

### Inputs the hook receives

| Parameter | Type | Source | Notes |
|---|---|---|---|
| `auth_type` | `common_enums::AuthenticationType` | `get_auth_type(&payload)` | Exactly two variants: `ThreeDs`, `NoThreeDs` (`crates/common/common_enums/src/enums.rs`, `pub enum AuthenticationType`); `NoThreeDs` is `#[default]`. |
| `payment_method` | `common_enums::PaymentMethod` | `get_payment_method(&payload)` | Hard error (`invalid_argument("missing payment_method")`) if absent. |
| `redirect_state` | `RedirectState` | `get_redirect_state(&payload)` | **Constant for the whole loop** — computed before `loop {`, never recomputed. |
| `completed_step` | `Option<AuthenticationStep>` | `state.completed_step` | The **most recent** completed leg only, not a set. Starts `None`. |

The connector instance the hook is called on comes from `ConnectorData::<domain_types::payment_method_data::DefaultPCIHolder>::get_connector_by_name(&connector)` — the composite path always resolves the `DefaultPCIHolder` instantiation. An override must therefore not depend on the generic `T`.

## The composite loop, step by step

Read `fn process_composite_authorize` in `crates/internal/composite-service/src/payments.rs`. The loop body is a four-arm `match` over the returned step. The arms are **not** symmetric, and the asymmetry is where connectors hang.

| Returned step | Sub-call | Sets `completed_step` | Breaks the loop when |
|---|---|---|---|
| `PreAuthenticate` | `self.pre_authenticate(..)` | `Some(PreAuthenticate)` | `r.redirection_data.is_some() \|\| is_failure_payment_status(r.status)` |
| `Authenticate` | `self.authenticate(..)` | `Some(Authenticate)` | `r.redirection_data.is_some() \|\| is_terminal_payment_status(r.status)` |
| `PostAuthenticate` | `self.post_authenticate(..)` | `Some(PostAuthenticate)` | **never** — control always returns to the top of the loop |
| `Authorize` | `self.authorize(..)` | — | unconditionally |

Three consequences follow directly and must be designed around:

1. **`PostAuthenticate` has no exit.** After it runs, the hook is called again with `completed_step = Some(PostAuthenticate)` and the *same* `redirect_state`. If your arms do not resolve that pair to `Authorize`, the loop calls `PostAuthenticate` forever. There is no iteration cap in the loop.
2. **`PreAuthenticate` and `Authenticate` use different break predicates.** `PreAuthenticate` breaks only on *failure*; `Authenticate` breaks on any *terminal* status. From `crates/internal/composite-service/src/utils.rs`:

```rust
// From crates/internal/composite-service/src/utils.rs
/// Check if payment status indicates a terminal state (success or failure)
pub fn is_terminal_payment_status(status: i32) -> bool {
    matches!(
        PaymentStatus::try_from(status).unwrap_or_default(),
        PaymentStatus::Charged
            | PaymentStatus::Authorized
            | PaymentStatus::PartialCharged
            | PaymentStatus::AuthenticationFailed
            | PaymentStatus::AuthorizationFailed
            | PaymentStatus::Failure
    )
}

/// Check if payment status indicates a failure state
pub fn is_failure_payment_status(status: i32) -> bool {
    matches!(
        PaymentStatus::try_from(status).unwrap_or_default(),
        PaymentStatus::AuthenticationFailed
            | PaymentStatus::AuthorizationFailed
            | PaymentStatus::Failure
    )
}
```

   So a `PreAuthenticate` that returns `Charged`/`Authorized` with **no** `redirection_data` does *not* break; the hook is consulted again. A `PreAuthenticate` returning neither a redirect nor a failure is a hook-driven decision point, not a loop exit.
3. **A break leaves `authorize_response` unset.** Breaking out of `PreAuthenticate` or `Authenticate` skips the `Authorize` arm entirely, so `CompositeAuthorizeResponse.authorize_response` is `None`. That is the intended shape of a challenge response: the caller redirects the customer and re-enters `CompositeAuthorize` with `redirection_response` populated, which flips `redirect_state` on the next call.

### How `CompositeStatus::RedirectRequired` is set

After the loop, the response builder computes:

```rust
// From crates/internal/composite-service/src/payments.rs — fn process_composite_authorize
        // Response construction - check if redirect occurred
        let has_redirection = state
            .pre_auth_response_opt
            .as_ref()
            .map(|r| r.redirection_data.is_some())
            .unwrap_or(false)
            || state
                .authn_response_opt
                .as_ref()
                .map(|r| r.redirection_data.is_some())
                .unwrap_or(false);

        let composite_status = if has_redirection {
            CompositeStatus::RedirectRequired
        } else {
            CompositeStatus::Completed
        };
```

Only `pre_authenticate_response` and `authenticate_response` are inspected. `post_authenticate_response.redirection_data` is **not** consulted — a `PostAuthenticate` that returns a redirect will still report `COMPLETED`. The proto (`crates/types-traits/grpc-api-types/proto/composite_payment.proto`, `enum CompositeStatus`) documents the caller contract: `REDIRECT_REQUIRED` means "redirect the customer, then call `CompositeAuthorize` again with `redirection_response`, or `CompositeVerifyRedirectResponse` for connectors that require a post-redirect authorize".

## Deriving RedirectState

`get_redirect_state` maps the presence and emptiness of one proto field onto the enum:

```rust
// From crates/internal/composite-service/src/payments.rs — fn get_redirect_state
    /// Derives redirect state from the proto redirection_response field.
    fn get_redirect_state(
        &self,
        payload: &CompositeAuthorizeRequest,
    ) -> interfaces::connector_types::RedirectState {
        match payload.redirection_response.as_ref() {
            None => interfaces::connector_types::RedirectState::InitialRequest,
            Some(r) => {
                if r.params.as_ref().map(|p| !p.is_empty()).unwrap_or(false) {
                    interfaces::connector_types::RedirectState::RedirectWithParams
                } else {
                    interfaces::connector_types::RedirectState::RedirectWithoutParams
                }
            }
        }
    }
```

The proto message it reads (`crates/types-traits/grpc-api-types/proto/payment.proto`, `message RedirectionResponse`):

```proto
message RedirectionResponse {
  optional string params = 1;
  map<string, string> payload = 2;
}
```

| Caller sends | Resulting `RedirectState` |
|---|---|
| no `redirection_response` at all | `InitialRequest` |
| `redirection_response` with non-empty `params` | `RedirectWithParams` |
| `redirection_response` with `params` absent **or** the empty string | `RedirectWithoutParams` |
| `redirection_response` with a populated `payload` map but empty `params` | `RedirectWithoutParams` — the `payload` map is **not** examined |

That last row is a live trap: an ACS that POSTs its result as form fields which the caller places in `payload` rather than `params` yields `RedirectWithoutParams`, and a connector whose arms key the challenge return on `RedirectWithParams` will fall through to its `_ => Authorize` catch-all and charge an unauthenticated card.

## Connectors with Full Implementation

This pattern documents a trait hook, not an HTTP flow, so the `PATTERN_AUTHORING_SPEC.md` §10 column set (HTTP method / content type / URL pattern) does not apply. The table below reports dispatch shape instead. Ordering is alphabetical.

| Connector | Gate condition | Dispatch shape | Legs implemented |
|---|---|---|---|
| `barclaycard.rs` | `ThreeDs && PaymentMethod::Card` | Full trio; branches on `(redirect_state, completed_step)` | Pre, Authn, Post |
| `cybersource.rs` | `ThreeDs && PaymentMethod::Card` | Identical arm-for-arm to Barclaycard | Pre, Authn, Post |
| `flywire.rs` | none — ignores `auth_type` and `payment_method` | `InitialRequest → Authenticate`, any redirect → `Authorize`; also sets `requires_authorize_post_redirect() = true` | Authn |
| `getnet.rs` | `ThreeDs && PaymentMethod::Card` | Pre → Authn in `InitialRequest`; frictionless exit after Authn; challenge return runs Post | Pre, Authn, Post |
| `grabpay.rs` | none — ignores `auth_type` and `payment_method` | Same shape as Flywire; also overrides `should_do_session_token` and `merchant_order_id_source` | Authn |
| `kount.rs` | none — ignores every parameter | Returns `PreAuthenticate` unconditionally | Pre |
| `moneris.rs` | `ThreeDs && matches!(pm, Card)` | Pre on initial; frictionless exit; any redirect return → `PostAuthenticate` | Pre, Post |
| `netcetera.rs` | `ThreeDs && PaymentMethod::Card` | Full trio with an explicit `(InitialRequest, Some(PreAuthenticate)) → Authorize` frictionless guard | Pre, Authn, Post |
| `paysafe.rs` | `ThreeDs && matches!(pm, Card \| Wallet)` | Pre on initial; both redirect states → Authn → Authorize | Pre, Authn |
| `redsys.rs` | `ThreeDs && PaymentMethod::Card` | Pre → Authn → Authorize entirely within `InitialRequest` | Pre, Authn |
| `worldpayxml.rs` | `ThreeDs && Card && InitialRequest` | Single `if`: `PreAuthenticate`, else `Authorize` | Pre |

### Implements 3DS legs but does NOT override the hook

These connectors have working `PreAuthenticate`/`Authenticate`/`PostAuthenticate` implementations that the composite loop will **never dispatch**. They are reachable only through the granular `PaymentMethodAuthenticationService` RPCs, driven by a caller that sequences the legs itself.

- `ilixium.rs` (PreAuthenticate)
- `nexixpay.rs` (PreAuthenticate, PostAuthenticate)
- `nmi.rs` (PreAuthenticate)
- `saferpay.rs` (PreAuthenticate)
- `worldpay.rs` (PreAuthenticate, PostAuthenticate)

### Deriving these rosters live

```bash
# who overrides the hook
rg -l "fn next_authentication_step" crates/integrations/connector-integration/src/connectors/

# who implements the legs
grep -n "flow_name: PreAuthenticate\|flow_name: Authenticate,\|flow_name: PostAuthenticate" \
  crates/integrations/connector-integration/src/connectors/*.rs
# at the pinned tree: PreAuthenticate 14, Authenticate 8, PostAuthenticate 7, overrides 11
```

## Common Implementation Patterns

### 1. The gate-then-match shape (recommended)

Every card-3DS override in the tree has the same two-level structure: an outer `if` that gates on `auth_type`/`payment_method` and returns `AuthenticationStep::Authorize` for everything else, and an inner `match (redirect_state, completed_step)` that encodes the state machine.

```rust
fn next_authentication_step(
    &self,
    auth_type: common_enums::AuthenticationType,
    payment_method: common_enums::PaymentMethod,
    redirect_state: connector_types::RedirectState,
    completed_step: Option<connector_types::AuthenticationStep>,
) -> connector_types::AuthenticationStep {
    use connector_types::{AuthenticationStep, RedirectState};
    if auth_type == common_enums::AuthenticationType::ThreeDs
        && payment_method == common_enums::PaymentMethod::Card
    {
        match (redirect_state, completed_step) {
            /* your state machine */
            _ => AuthenticationStep::Authorize,
        }
    } else {
        AuthenticationStep::Authorize
    }
}
```

The outer `else` is not optional decoration: without it, a `NoThreeDs` card payment would enter the 3DS legs.

### 2. The redirect-only shape (no 3DS at all)

Flywire and Grabpay reuse the hook for a non-3DS hosted-page journey: the `Authenticate` leg opens a checkout session and returns the iframe/redirect; the customer's return re-enters as `Authorize`. Both ignore `auth_type` and `payment_method` entirely and pair the override with `requires_authorize_post_redirect() -> true`, which is read in `fn process_composite_verify_redirect_response` in the same `payments.rs` to run a post-redirect Authorize.

### 3. The single-leg shape

`worldpayxml.rs` and `kount.rs` need only one leg. Worldpayxml returns `PreAuthenticate` for the `InitialRequest`-and-card-3DS case and `Authorize` otherwise; Kount returns `PreAuthenticate` unconditionally and relies on the `PreAuthenticate` arm's `redirection_data` break to terminate the loop.

## Connector-Specific Patterns

### barclaycard.rs / cybersource.rs — the canonical trio

Both files carry the identical arm set (see [Code Examples](#code-examples)). The state machine reads: `InitialRequest` always means "run DDC"; a return **with** params is the AReq leg; a return **without** params is the challenge-result leg. Note the first arm is `(RedirectState::InitialRequest, _)` — it ignores `completed_step`, so termination depends entirely on `PreAuthenticate` returning `redirection_data` (or a failure status) on the initial call. See gotcha 2.

### netcetera.rs / getnet.rs — the guarded initial arm

Netcetera splits the initial state in two:

```rust
// From crates/integrations/connector-integration/src/connectors/netcetera.rs — fn next_authentication_step
                // Initial request, nothing done yet: run the 3DS version / method call.
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,

                // PreAuthenticate completed with no browser redirect needed (e.g. only
                // 3DS1 supported, or the vault-token/card_proxy path skips DDC) — proceed
                // straight to authorize instead of looping back into PreAuthenticate again.
                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authorize
                }
```

This is the safe form of the Barclaycard first arm and the one new connectors should copy. Getnet uses the same guard but routes the second state to `Authenticate` rather than `Authorize`, giving a full Pre → Authn chain inside a single `InitialRequest` call, with `(InitialRequest, Some(Authenticate)) => Authorize` as the frictionless exit.

`netcetera.rs` is the authentication-only connector — it carries a stub Authorize plus the full trio and lives in the payment connector registry, not the authenticator one.

### moneris.rs — challenge-return-driven, with a shadowed arm

```rust
// From crates/integrations/connector-integration/src/connectors/moneris.rs — fn next_authentication_step
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, None) => AuthenticationStep::PreAuthenticate,
                // Frictionless: PreAuthenticate completed with no redirect → go straight to Authorize
                (RedirectState::InitialRequest, Some(AuthenticationStep::PreAuthenticate)) => {
                    AuthenticationStep::Authorize
                }
                // Challenge: ACS posted cres back → run PostAuthenticate (auth value lookup)
                (RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams, _) => {
                    AuthenticationStep::PostAuthenticate
                }
                // After PostAuthenticate → Authorize
                (_, Some(AuthenticationStep::PostAuthenticate)) => AuthenticationStep::Authorize,
                _ => AuthenticationStep::Authorize,
            }
```

Moneris skips `Authenticate` entirely: the ACS challenge is opened by `PreAuthenticate`'s redirect, and the return runs `PostAuthenticate` for the authentication-value lookup. **Do not copy the arm ordering.** Rust matches arms in order, and the third arm's `_` in the `completed_step` position swallows `Some(PostAuthenticate)` for both redirect states — so the fourth arm is reachable only when `redirect_state == InitialRequest`, a combination the earlier arms make unreachable in practice. Put `completed_step`-bearing arms **before** any catch-all redirect arm. See gotcha 4.

### paysafe.rs — wallets on the card path

Paysafe widens the gate to `matches!(payment_method, PaymentMethod::Card | PaymentMethod::Wallet)`. Its in-source rationale: *"a Google Pay token with no cryptogram is non-SCA on its own, and Paysafe's guidance is to authenticate it with a 3DS challenge rather than skip 3DS"*. This is the reference for any connector whose wallet payload must ride the 3DS chain.

### redsys.rs — the whole trio inside `InitialRequest`

Redsys drives Pre → Authn → Authorize without leaving `InitialRequest`, and maps `(RedirectWithoutParams, None) => Authorize` so a paramless return settles directly. It is the shortest full example of chaining two legs on `completed_step` alone.

### flywire.rs / grabpay.rs — hosted checkout, not 3DS

```rust
// From crates/integrations/connector-integration/src/connectors/flywire.rs — fn next_authentication_step
    /// Flywire's flow: Authenticate (POST /checkout/sessions → iframe) on the
    /// initial request; Authorize (POST /checkout/sessions/{id}/confirm) after
    /// the customer completes payment and the caller redirects back.
    fn next_authentication_step(
        &self,
        _auth_type: common_enums::AuthenticationType,
        _payment_method: common_enums::PaymentMethod,
        redirect_state: RedirectState,
        _completed_step: Option<AuthenticationStep>,
    ) -> AuthenticationStep {
        match redirect_state {
            RedirectState::InitialRequest => AuthenticationStep::Authenticate,
            RedirectState::RedirectWithParams | RedirectState::RedirectWithoutParams => {
                AuthenticationStep::Authorize
            }
        }
    }

    fn requires_authorize_post_redirect(&self) -> bool {
        true
    }
```

Grabpay's override carries the same three arms (its signature and `use` spell the types through `connector_types::`/`interfaces::connector_types::` rather than bare); it differs in the sibling `ValidationTrait` overrides it adds (`should_do_session_token`, `requires_authorize_post_redirect`, `merchant_order_id_source`). Termination here relies on the `Authenticate` arm's break: the session response carries `redirection_data`, so the loop exits and `composite_status` becomes `RedirectRequired`.

### kount.rs — FRM device data collection, not authentication

```rust
// From crates/integrations/connector-integration/src/connectors/kount.rs — fn next_authentication_step
        // Kount only runs PreAuthenticate (DDC); the composite loop breaks once
        // the DDC `redirection_data` is present. FRM risk checks run separately
        // via the FraudAndRiskManagementService composite flow.
        connector_types::AuthenticationStep::PreAuthenticate
```

The unconditional return is safe only because Kount's `PreAuthenticate` always emits `redirection_data`: `handle_pre_authenticate_response` in `connectors/kount/transformers.rs` builds `PreAuthenticateResponse { redirection_data: Some(Box::new(RedirectForm::Script { .. })), .. }` on every path, with no branch that leaves it `None`. Copying this shape onto a connector whose PreAuthenticate can return without a redirect produces an infinite loop.

## Code Examples

### The canonical trio state machine — `barclaycard.rs`, verbatim

```rust
// From crates/integrations/connector-integration/src/connectors/barclaycard.rs
// impl<T: ...> connector_types::ValidationTrait for Barclaycard<T>
    fn next_authentication_step(
        &self,
        auth_type: common_enums::AuthenticationType,
        payment_method: common_enums::PaymentMethod,
        redirect_state: connector_types::RedirectState,
        completed_step: Option<connector_types::AuthenticationStep>,
    ) -> connector_types::AuthenticationStep {
        use connector_types::{AuthenticationStep, RedirectState};
        if auth_type == common_enums::AuthenticationType::ThreeDs
            && payment_method == common_enums::PaymentMethod::Card
        {
            match (redirect_state, completed_step) {
                (RedirectState::InitialRequest, _) => AuthenticationStep::PreAuthenticate,

                (RedirectState::RedirectWithParams, None) => AuthenticationStep::Authenticate,

                (RedirectState::RedirectWithParams, Some(AuthenticationStep::Authenticate)) => {
                    AuthenticationStep::Authorize
                }

                (RedirectState::RedirectWithoutParams, None) => {
                    AuthenticationStep::PostAuthenticate
                }

                (
                    RedirectState::RedirectWithoutParams,
                    Some(AuthenticationStep::PostAuthenticate),
                ) => AuthenticationStep::Authorize,

                _ => AuthenticationStep::Authorize,
            }
        } else {
            AuthenticationStep::Authorize
        }
    }
```

Traced against the loop, over three separate gRPC calls:

| gRPC call | `redirect_state` | Iteration | `completed_step` in | Step out | Outcome |
|---|---|---|---|---|---|
| 1 | `InitialRequest` | 1 | `None` | `PreAuthenticate` | DDC form returned → `redirection_data.is_some()` → **break**, `composite_status = REDIRECT_REQUIRED` |
| 2 (after DDC POST, params present) | `RedirectWithParams` | 1 | `None` | `Authenticate` | frictionless: no redirect, non-terminal status → loop continues |
| 2 | `RedirectWithParams` | 2 | `Some(Authenticate)` | `Authorize` | charge, **break**, `COMPLETED` |
| 2′ (challenge branch) | `RedirectWithParams` | 1 | `None` | `Authenticate` | ACS challenge → `redirection_data.is_some()` → **break**, `REDIRECT_REQUIRED` |
| 3 (after challenge, no params) | `RedirectWithoutParams` | 1 | `None` | `PostAuthenticate` | CRes validated; arm has no break → loop continues |
| 3 | `RedirectWithoutParams` | 2 | `Some(PostAuthenticate)` | `Authorize` | charge with CAVV/ECI, **break**, `COMPLETED` |

The `(RedirectWithoutParams, Some(PostAuthenticate))` arm is what keeps row 6 from re-running `PostAuthenticate` forever. Delete it and the connector hangs.

### The `else` branch is load-bearing

A caller that omits `auth_type` lands in the `else` branch and gets a plain Authorize. `get_auth_type` applies `unwrap_or_default()` to the proto value, which yields `AUTHENTICATION_TYPE_UNSPECIFIED = 0` (`crates/types-traits/grpc-api-types/proto/payment.proto`, `enum AuthenticationType`), and the `ForeignTryFrom<grpc_api_types::payments::AuthenticationType>` impl in `crates/types-traits/domain_types/src/types.rs` maps `Unspecified => Ok(Self::NoThreeDs)`. That fallback is the safe one, and it is why the gate must be an explicit equality test against `ThreeDs` rather than a negation.

## Integration Guidelines

1. **Decide whether you need the hook at all.** You need it if the connector performs gateway-side 3DS (its own `PreAuthenticate`/`Authenticate`/`PostAuthenticate` endpoints) *or* uses a hosted-checkout redirect driven by the `Authenticate` leg. You do **not** need it for external 3DS (Netcetera-as-a-service in the router, 3dsecure.io, Gpayments, Cardinal, CTP) — that class never reaches UCS. You do not need it for merchant/credential authentication (`ServerAuthenticationToken`, `ServerSessionAuthenticationToken`, `ClientAuthenticationToken`), which is a different mechanism on `MerchantAuthenticationFlowData` and a different gRPC service.
2. **Find the `ValidationTrait` impl the scaffold emitted.** It is the empty block under `===== BASE (NON-FLOW) TRAIT IMPLEMENTATIONS =====` in your connector's main file, generated from `grace/rulesbook/codegen/add_connector.sh`.
3. **Add `next_authentication_step` into that existing impl block.** Do not open a second `impl ValidationTrait` (E0119). Signature must match the trait exactly, including parameter order `(auth_type, payment_method, redirect_state, completed_step)`.
4. **Write the gate.** `if auth_type == common_enums::AuthenticationType::ThreeDs && payment_method == common_enums::PaymentMethod::Card { .. } else { AuthenticationStep::Authorize }`. Widen the payment-method test only with a documented reason, as `paysafe.rs` does for wallets.
5. **Write the arms** using the [decision table](#decision-table-choosing-your-arms). Order `completed_step`-bearing arms before catch-all arms.
6. **Prove termination.** For every `(redirect_state, completed_step)` pair your arms can produce, confirm the successor either breaks the loop or advances toward `Authorize`. Pay special attention to `PostAuthenticate`, which never breaks.
7. **End with `_ => AuthenticationStep::Authorize`.** Never `unreachable!()` or a panic — the hook runs inside the request path.
8. **Register the legs.** The hook only *selects*; the leg must also exist. For each step your arms can return:
   - Remove that flow's name (`PreAuthenticate` / `Authenticate` / `PostAuthenticate`) from the `not_implemented:` list of the connector's `macros::macro_connector_flow_status_impls!` invocation. The scaffold puts all three there (`get_flow_name_for_trait` in `grace/rulesbook/codegen/add_connector.sh` maps `PaymentPreAuthenticateV2 → PreAuthenticate`), and the `not_implemented` arm of `flow_status_emit!` in `crates/integrations/connector-integration/src/connectors/macros.rs` already emits both the marker-trait impl and a `ConnectorIntegrationV2` stub. Leaving the name in place while adding a real block is a duplicate `ConnectorIntegrationV2` impl — E0119. `barclaycard.rs` is the reference: its `macro_connector_flow_status_impls!` list carries none of the three.
   - Add the marker-trait impl by hand: `PaymentPreAuthenticateV2<T>` / `PaymentAuthenticateV2<T>` / `PaymentPostAuthenticateV2<T>` (all three bind `PaymentFlowData` as their `ResourceCommonData`).
   - Add the `macro_connector_implementation!` block — or `macro_connector_local_flow_implementation!` for a leg with no outbound HTTP call, as `kount.rs` does for `PreAuthenticate`.

   Returning a step the connector left in `not_implemented:` is **not** a compile error: the stub's `get_url` returns `IntegrationError::connector_flow_not_implemented(..)`, i.e. the `NotImplemented` variant, at runtime.
9. **File the router-side change.** See [The router-side gate](#the-router-side-gate). Without it the integration stops after leg 1 in the real product regardless of what UCS does.

## Decision table: choosing your arms

Answer these against the vendor's 3DS documentation, then read the arms off the table.

| Question about the gateway | Arm to write |
|---|---|
| Does the first call need device data collection / a 3DS method URL? | `(InitialRequest, None) => PreAuthenticate` |
| Can that first call complete without a browser redirect (3DS1-only, vaulted token, frictionless BIN)? | add `(InitialRequest, Some(PreAuthenticate)) => Authorize` — **or** `=> Authenticate` if an AReq must still run (Getnet, Redsys) |
| Is there a separate enrolment/AReq call after DDC returns? | `(RedirectWithParams, None) => Authenticate` (Barclaycard, Cybersource) or `(InitialRequest, Some(PreAuthenticate)) => Authenticate` (Getnet, Redsys) |
| Does the AReq resolve frictionless (CAVV in the response, no ACS)? | `(<same state>, Some(Authenticate)) => Authorize` |
| Does the customer return from the ACS with the result in the query/body (`params` non-empty)? | `(RedirectWithParams, ..) => PostAuthenticate` — then `(RedirectWithParams, Some(PostAuthenticate)) => Authorize` |
| Does the customer return with nothing but the return URL (`params` empty/absent)? | `(RedirectWithoutParams, None) => PostAuthenticate` — then `(RedirectWithoutParams, Some(PostAuthenticate)) => Authorize` |
| Are you unsure which of the two return states the caller will produce? | Handle both with an or-pattern, e.g. `(RedirectWithParams \| RedirectWithoutParams, None) => ..` (Paysafe, Moneris) — and put the `Some(..)` arms first |
| Is there no 3DS at all, just a hosted checkout page? | `InitialRequest => Authenticate`; both redirect states `=> Authorize`; add `requires_authorize_post_redirect() -> true` (Flywire, Grabpay) |
| Does the connector only need one leg and always redirect from it? | Return that step from a single `if`, else `Authorize` (Worldpayxml); unconditional return only if that leg *always* redirects or fails (Kount) |
| Any pair not covered above | `_ => AuthenticationStep::Authorize` |

Termination check, to run once the arms are written: enumerate the 12 reachable `(RedirectState, Option<AuthenticationStep>)` pairs — 3 redirect states × 4 reachable `completed_step` values. The type admits 5 (`None` plus one per `AuthenticationStep` variant), but `Some(AuthenticationStep::Authorize)` never occurs: the loop's `Authorize` arm in `fn process_composite_authorize` breaks without assigning `state.completed_step`. For each, apply your arms and then the loop table in [The composite loop](#the-composite-loop-step-by-step). Any cycle that does not pass through a break is a hang.

## The router-side gate

UCS's `next_authentication_step` decides what runs **within one `CompositeAuthorize` call**. It has no authority over whether the Hyperswitch router issues the *next* call. That decision lives in the router repo:

- File: `crates/router/src/core/payments/flows/authorize_flow.rs` (hyperswitch repo — **not present in this tree**; `ls crates/router` returns nothing here).
- Functions: `should_continue_after_preauthenticate`, `should_continue_after_authenticate`.
- Shape: a per-connector match whose **default is `false`**.

The consequence for a newly generated connector: leg 1 executes, returns its redirect, and the journey stops. No error is raised — the router simply does not continue — which is why this presents as "3DS silently does nothing" rather than as a failure. The in-tree corroboration is the doc comment on `fn is_three_ds_settlement` in `crates/integrations/connector-integration/src/connectors/saferpay/transformers.rs`:

> `PreAuthenticate` opens the journey with `Initialize` and returns a redirect; the caller stops there (`should_continue_after_preauthenticate` defaults to false), so a 3DS attempt never reaches Authorize before the shopper has been away.

Treat this as a two-repo deliverable. A UCS PR that adds a 3DS trio plus a dispatch override is **not** a complete 3DS integration; the paired router change must be filed and tracked, and the connector's 3DS scenarios cannot be certified end-to-end until it lands. Because the file is outside this repository, none of its contents can be verified from here — do not fabricate arm names, connector-enum spellings, or line numbers for it. Read it in the hyperswitch checkout.

## Best Practices

- Put the override in the **existing** `impl connector_types::ValidationTrait for <Connector><T>` block that the scaffold generated (`grace/rulesbook/codegen/add_connector.sh`, base-traits block), never in a new impl.
- Copy the guarded initial arm from `netcetera.rs` (`(InitialRequest, None)` plus `(InitialRequest, Some(PreAuthenticate))`) rather than the unguarded `(InitialRequest, _)` from `barclaycard.rs`, unless your `PreAuthenticate` is proven to always redirect or fail.
- Order match arms most-specific first. Every arm binding `Some(step)` in the `completed_step` position must precede any arm with `_` there — the `moneris.rs` ordering shows what happens otherwise.
- Always terminate with `_ => AuthenticationStep::Authorize`; the hook is on the hot request path and must not panic.
- Keep the override free of `T`: the composite path resolves the connector as `ConnectorData::<DefaultPCIHolder>` (`fn process_composite_authorize`), so per-`T` behaviour here is not reachable.
- Document each non-obvious arm with a one-line comment naming the gateway concept it maps to. `netcetera.rs`, `moneris.rs` and `getnet.rs` all do this and are the most readable overrides in the tree.
- Do not use this hook for merchant/credential authentication. `ServerAuthenticationToken`, `ServerSessionAuthenticationToken` and `ClientAuthenticationToken` bind `MerchantAuthenticationFlowData` (`crates/types-traits/domain_types/src/merchant_authentication_flow_data.rs`) on `MerchantAuthenticationService` and are sequenced by `create_server_authentication_token` / `create_server_session_authentication_token`, which run **before** the loop.

## Common Errors / Gotchas

1. **Problem:** The trio compiles, unit tests pass, and every payment settles as `NoThreeDs` — the legs are never called.
   **Solution:** The `ValidationTrait` impl is still the scaffold's empty block, so `next_authentication_step` returns the default `AuthenticationStep::Authorize`. Add the override. Verify with `rg -n "fn next_authentication_step" crates/integrations/connector-integration/src/connectors/<name>.rs`.

2. **Problem:** The composite call never returns; the service pegs a core issuing `PreAuthenticate` requests in a tight loop.
   **Solution:** An arm returns `PreAuthenticate` for a state that persists after the leg completes — the `(RedirectState::InitialRequest, _)` shape — while the leg returns neither `redirection_data` nor a failure status, so the `PreAuthenticate` break never fires and `redirect_state` never changes. Add the `(InitialRequest, Some(PreAuthenticate)) => ..` guard as `netcetera.rs` does.

3. **Problem:** Same hang, but on `PostAuthenticate`.
   **Solution:** The `PostAuthenticate` arm in the loop has **no** break of any kind. You must supply an arm resolving `(<that redirect_state>, Some(PostAuthenticate))` to `Authorize`.

4. **Problem:** The `(_, Some(AuthenticationStep::PostAuthenticate)) => Authorize` arm you wrote is never taken.
   **Solution:** An earlier arm with `_` in the `completed_step` position shadows it — the `moneris.rs` ordering. Rust does not warn here, because the arm remains reachable for at least one pair. Move the `Some(..)` arms above the catch-all.

5. **Problem:** The challenge return charges the card unauthenticated instead of running `PostAuthenticate`.
   **Solution:** `get_redirect_state` classifies on `redirection_response.params` **only**; a return whose data arrived in the `payload` map (or as an empty `params` string) is `RedirectWithoutParams`, and your `RedirectWithParams`-keyed arm falls through to `_ => Authorize`. Handle both return states with an or-pattern unless you control the caller.

6. **Problem:** The hook returns a step, and the response comes back `IntegrationError::NotImplemented`.
   **Solution:** Selecting a step does not implement it. Add the `macro_connector_implementation!` block and the corresponding `PaymentPreAuthenticateV2<T>`/`PaymentAuthenticateV2<T>`/`PaymentPostAuthenticateV2<T>` impl. Nothing in the type system ties the returned enum variant to the presence of the flow.

7. **Problem:** A challenge is issued but `composite_status` comes back `COMPLETED`, so the caller never redirects.
   **Solution:** `has_redirection` inspects only `pre_auth_response_opt` and `authn_response_opt`. A redirect emitted from `PostAuthenticate` is invisible to it. Emit challenge redirects from `PreAuthenticate` or `Authenticate`.

8. **Problem:** The first call works in the product; the second never arrives.
   **Solution:** The router-side gate, not UCS. See [The router-side gate](#the-router-side-gate) — `should_continue_after_preauthenticate` defaults to `false`.

9. **Problem:** `NoThreeDs` payments start hitting the 3DS endpoints after the override lands.
   **Solution:** The outer gate is missing or inverted. An unset proto `auth_type` resolves to `NoThreeDs` (`Unspecified => Ok(Self::NoThreeDs)` in `crates/types-traits/domain_types/src/types.rs`), so test `== AuthenticationType::ThreeDs` explicitly and return `Authorize` from the `else`.

10. **Problem:** E0119, conflicting implementations, after wiring the dispatch.
    **Solution:** Two sources. (a) A second `impl connector_types::ValidationTrait for <Connector><T>` block — merge the override into the scaffold's existing one. (b) A leg implemented with `macro_connector_implementation!` while its flow name is still listed in `not_implemented:` of `macros::macro_connector_flow_status_impls!`, which emits its own `ConnectorIntegrationV2` stub for that flow — remove the name from the list (Integration Guidelines step 8).

## Testing Notes

### Unit tests

The hook is a pure function of four `Copy` inputs and needs no HTTP, so test it directly in the connector's own test module — `connectors/<name>/test.rs` is the convention in this tree (`adyen/test.rs`, `razorpay/test.rs`, `calida/test.rs`), not `tests.rs`:

- Assert the full transition table: for each `(RedirectState, Option<AuthenticationStep>)` pair your gate admits, assert the returned `AuthenticationStep`. All three enums derive `Debug, Clone, Copy, PartialEq, Eq`, so `assert_eq!` works without helpers.
- Assert `AuthenticationType::NoThreeDs` returns `AuthenticationStep::Authorize` for every redirect state.
- Assert every non-card payment method your gate excludes returns `AuthenticationStep::Authorize`.
- Assert the termination property explicitly: for each pair, the returned step is either `Authorize` or a step whose loop arm can break given the response your transformers produce. A test that walks the 12 reachable pairs (see the [decision table](#decision-table-choosing-your-arms) for why `Some(Authorize)` is not one of them) and asserts no `(state, step)` maps to itself catches gotchas 2–4 mechanically.

### Integration test scenarios

| Scenario | Inputs | Expected output |
| --- | --- | --- |
| Frictionless card 3DS | `auth_type = THREE_DS`, card, no `redirection_response` | Loop runs the connector's initial leg(s) and exits via `Authorize`; `composite_status = COMPLETED`; `authorize_response` populated. |
| Challenge, leg 1 | `auth_type = THREE_DS`, challenge BIN, no `redirection_response` | Breaks on `redirection_data`; `composite_status = REDIRECT_REQUIRED`; `authorize_response` is `None`. |
| Challenge return with params | same request plus `redirection_response.params = "<ACS body>"` | `RedirectWithParams` branch runs; ends in `Authorize`; `COMPLETED`. |
| Challenge return without params | same request plus `redirection_response` with empty `params` | `RedirectWithoutParams` branch runs; ends in `Authorize`; `COMPLETED`. |
| Non-3DS regression | `auth_type = NO_THREE_DS`, same card | No authentication sub-response is populated at all; `COMPLETED`. |
| Authentication declined | ACS returns denial | Leg breaks on `is_failure_payment_status` / `is_terminal_payment_status`; no `Authorize` call is made. |

Exercise these through `CompositePaymentService.Authorize` (`crates/types-traits/grpc-api-types/proto/composite_services.proto`); the granular `PaymentService.Authorize` path does not consult this hook and will not reproduce the behaviour.

### Certification note

3DS scenarios declared in the connector's `specs.json` are validated by `.github/scripts/verify-new-connectors.sh`, which is merge-blocking for connectors whose spec directory did not exist at the merge base. A declared 3DS scenario that fails because the dispatch override is missing — or because the router-side gate has not landed — hard-fails the check.

## Cross-References

- Parent index: [./README.md](./README.md)
- The three legs this hook dispatches: [pattern_preauthenticate.md](./pattern_preauthenticate.md), [pattern_authenticate.md](./pattern_authenticate.md), [pattern_postauthenticate.md](./pattern_postauthenticate.md)
- Terminal step: [pattern_authorize.md](./pattern_authorize.md); card 3DS payload shaping: [authorize/card/pattern_authorize_card.md](./authorize/card/pattern_authorize_card.md)
- A *different* mechanism, do not conflate: [pattern_server_authentication_token.md](./pattern_server_authentication_token.md), [pattern_server_session_authentication_token.md](./pattern_server_session_authentication_token.md), [pattern_client_authentication_token.md](./pattern_client_authentication_token.md) — merchant/credential auth on `MerchantAuthenticationFlowData`, `MerchantAuthenticationService`.
- Authoring spec: [./PATTERN_AUTHORING_SPEC.md](./PATTERN_AUTHORING_SPEC.md)
- Source anchors (symbols, not lines — the numeric citations in the older auth patterns have rotted):
  - `crates/types-traits/interfaces/src/connector_types.rs` — `pub enum AuthenticationStep`, `pub enum RedirectState`, `pub trait ValidationTrait::next_authentication_step`, `::requires_authorize_post_redirect`
  - `crates/internal/composite-service/src/payments.rs` — `fn process_composite_authorize`, `fn get_redirect_state`, `fn get_auth_type`, `fn get_payment_method`, `struct AuthorizeCompositeState`
  - `crates/internal/composite-service/src/utils.rs` — `pub fn is_terminal_payment_status`, `pub fn is_failure_payment_status`
  - `crates/types-traits/grpc-api-types/proto/composite_payment.proto` — `message CompositeAuthorizeRequest`, `enum CompositeStatus`, `message CompositeAuthorizeResponse`
  - `crates/types-traits/grpc-api-types/proto/payment.proto` — `message RedirectionResponse`
  - `crates/common/common_enums/src/enums.rs` — `pub enum AuthenticationType`
  - Overrides: `connectors/{barclaycard,cybersource,flywire,getnet,grabpay,kount,moneris,netcetera,paysafe,redsys,worldpayxml}.rs` — `fn next_authentication_step`
