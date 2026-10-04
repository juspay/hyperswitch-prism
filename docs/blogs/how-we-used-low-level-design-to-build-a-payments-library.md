# How We Used Low-Level Design to Build a Payments Library for 100+ Processors

Every payment processor does the same job. Take money from a card, hold it, capture it, refund it. None of them agree on how.

Stripe calls it a PaymentIntent. Adyen calls it a payment. PayPal calls it an order. One returns `"Captured"`, another returns `"SETTLED"`, a third returns `"1"`. Prism supports **97 of them** behind a single API.

That number is the whole design problem. With 97 integrations, any decision you get wrong doesn't cost you once — it costs you 97 times, and again for every processor you add. This post walks through the low-level design that keeps the 98th connector from turning into a rewrite: the OOP fundamentals, the class relationships, and the design patterns — each one with the actual code.

The interesting part isn't that we used them. It's that at 97 connectors you don't get to skip them.

---

## The naive version, and why it dies

The obvious first design is a function with a switch in it.

```rust
match connector {
    "stripe" => { /* build Stripe's JSON, call Stripe, parse Stripe's response */ }
    "adyen"  => { /* build Adyen's JSON, call Adyen, parse Adyen's response */ }
    // … 95 more
}
```

This works fine at two connectors. At ten it's unpleasant. At 97 it's a building on fire: every processor's quirks are smeared through the core, every new integration edits code all 96 others depend on, and one bad merge takes down payments for everybody.

The fix is the oldest idea in object orientation — put a seam between *what* a payment does and *how* a given processor does it. Everything below is a consequence of that one move.

## First, a note on `<T>`

Almost every type in this post carries a `<T: PaymentMethodDataTypes + …>` bound, so it's worth thirty seconds up front. `T` decides how card data is represented, at compile time:

```rust
pub trait PaymentMethodDataTypes: Clone {
    type Inner: Default + Debug + Send + Eq + PartialEq + Serialize + DeserializeOwned + Clone;
    fn peek_inner(inner: &Self::Inner) -> &str;
}

pub struct DefaultPCIHolder;   // Inner = cards::CardNumber  — raw PAN
pub struct VaultTokenHolder;   // Inner = Secret<String>     — vault token
```

[`crates/types-traits/domain_types/src/payment_method_data.rs:138`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/domain_types/src/payment_method_data.rs#L138)

Two zero-sized markers. `ConnectorData<DefaultPCIHolder>` and `ConnectorData<VaultTokenHolder>` are *different types*, so the raw-card path and the tokenized path can't be crossed by accident — the type checker won't allow it, and it costs nothing at runtime. We've elided the bounds in the snippets below.

---

# OOP Fundamentals

## Encapsulation

Start here, because in payments this is the one that bites. The card number's field is private, and every accessor hands back the minimum a caller could need:

```rust
pub struct CardNumber(StrongSecret<String, CardNumberStrategy>);   // field is private

impl CardNumber {
    pub fn get_card_isin(&self) -> String {
        self.0.peek().chars().take(6).collect::<String>()
    }
    pub fn get_last4(&self) -> String { /* … */ }
}
```

[`crates/types-traits/cards/src/validate.rs:22`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/cards/src/validate.rs#L22)

You can't reach the PAN by accident. You ask for the ISIN, or the last four, and that's what you get. Construction runs Luhn, length, and charset checks, so every `CardNumber` in the system is valid *because it exists* — parse, don't validate.

Then the part that saves you at 3am:

```rust
pub enum CardNumberStrategy {}   // uninhabited — can never be constructed

impl<T> Strategy<T> for CardNumberStrategy where T: AsRef<str> {
    fn fmt(val: &T, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let val_str: &str = val.as_ref();
        if val_str.len() < 15 || val_str.len() > 19 {
            return WithType::fmt(val, f);
        }
        if let Some(value) = val_str.get(..6) {
            write!(f, "{}{}", value, "*".repeat(val_str.len() - 6))
        } else {
            WithType::fmt(val, f)
        }
    }
}
```

[`crates/types-traits/cards/src/validate.rs:291`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/cards/src/validate.rs#L291)

An enum with **zero variants**. It can never be instantiated; it exists only as a compile-time tag that selects how the value formats. Log a card by mistake and the log gets `411111**********`. Encapsulation enforced at the boundary where it actually tends to fail.

## Enums

This is the one to look at. A single enum models roughly a hundred mutually exclusive credential schemas — and each variant carries a *different shape*:

```rust
pub enum ConnectorSpecificConfig {
    /// No credentials required.
    NoKey,
    Stripe { api_key: Secret<String>, base_url: Option<String> },
    Calida { api_key: Secret<String>, base_url: Option<String> },
    Mifinity { key: Secret<String>, base_url: Option<String> },
    // … ~100 variants
}
```

[`crates/types-traits/domain_types/src/router_data.rs:301`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/domain_types/src/router_data.rs#L301)

This is a sum type, not a C-style enum. You cannot construct a Stripe config with Mifinity's fields, because that combination has no representation. Illegal states aren't rejected at runtime — they're unspellable.

Enums also carry behaviour, so domain logic lives on the type instead of scattered across callers:

```rust
impl AttemptStatus {
    pub fn is_terminal_status(self) -> bool {
        matches!(
            self,
            Self::Charged | Self::AutoRefunded | Self::Voided | Self::VoidedPostCapture
                | Self::PartialCharged | Self::AuthenticationFailed | Self::AuthorizationFailed
                | Self::VoidFailed | Self::CaptureFailed | Self::Failure | Self::IntegrityFailure
        )
    }
}
```

[`crates/common/common_enums/src/enums.rs:1426`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/common/common_enums/src/enums.rs#L1426)

"Is this payment finished?" is asked in a dozen places. It's answered in one.

## Interfaces and abstraction

Two traits, and between them they're the whole seam. The first is an abstract base class in everything but name — two pure-virtual methods, plus hooks with sensible behaviour you only touch when your processor is weird:

```rust
pub trait ConnectorCommon {
    /// Name of the connector (in lowercase).
    fn id(&self) -> &'static str;                                    // required

    /// The base URL for interacting with the connector's API.
    fn base_url<'a>(&self, connectors: &'a Connectors) -> &'a str;   // required

    fn get_currency_unit(&self) -> CurrencyUnit {
        CurrencyUnit::Minor                                          // defaulted
    }

    fn common_get_content_type(&self) -> &'static str {
        "application/json"                                           // defaulted
    }
}
```

[`crates/types-traits/interfaces/src/api.rs:15`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/api.rs#L15)

The second is the abstraction the entire library turns on — one trait, generic over the flow, the request, and the response:

```rust
pub trait ConnectorIntegrationV2<Flow, ResourceCommonData, Req, Resp>:
    ConnectorIntegrationAnyV2<Flow, ResourceCommonData, Req, Resp> + Sync + api::ConnectorCommon
{
    fn get_url(&self, _req: &RouterDataV2<…>) -> CustomResult<String, IntegrationError>;

    fn get_headers(&self, …) -> … { Ok(vec![]) }
    fn get_http_method(&self) -> Method { Method::Post }
    fn get_request_body(&self, …) -> … { Ok(None) }
}
```

[`crates/types-traits/interfaces/src/connector_integration_v2.rs:45`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/connector_integration_v2.rs#L45)

Note what's required: **`get_url`, and nothing else.** A connector that speaks JSON over POST overrides almost nothing. Authorize, Capture, and Refund aren't three interfaces — they're the same interface with different type parameters.

## Inheritance

Rust has no class inheritance, and we're not going to pretend otherwise. What it has is *interface* inheritance through supertraits:

```rust
pub trait ConnectorServiceTrait<T>:
    ConnectorCommon
    + ValidationTrait
    + PaymentAuthorizeV2<T>
    + PaymentSyncV2
    + RefundV2
    + PaymentCapture
    // … ~30 supertraits
{
}
```

[`crates/types-traits/interfaces/src/connector_types.rs:91`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/connector_types.rs#L91)

The body is empty. It inherits ~30 capabilities and adds nothing of its own. `impl ConnectorServiceTrait for Adyen {}` compiles only if Adyen satisfies every one of them.

Where you'd reach for a base class with default method bodies, we generate them instead — `default_implementations.rs` macro-emits no-op impls so connectors "inherit" behaviour they didn't write. Worth being precise: overriding there means the macro doesn't emit the default. It isn't virtual dispatch.

---

# Class Relationships

Five relationships, five pieces of real code.

## Association

A handle paired with its identity. `ConnectorData` doesn't own the connector's storage — the box wraps a `&'static` reference — so this is association, not composition:

```rust
pub struct ConnectorData<T> {
    pub connector: BoxedConnector<T>,
    pub connector_name: ConnectorEnum,
}
```

[`crates/integrations/connector-integration/src/types.rs:16`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/types.rs#L16)

Keeping `connector_name` alongside the type-erased handle preserves identity the vtable would otherwise lose.

## Aggregation

`Config` is shared, not owned. Many layers point at one config, and it outlives all of them:

```rust
pub struct RequestExtensionsLayer {
    base_config: Arc<Config>,
}
```

[`crates/grpc-server/grpc-server/src/config_overrides.rs:13`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/grpc-server/grpc-server/src/config_overrides.rs#L13)

## Composition

The app state owns thirteen services *by value*. Drop it and they all go with it:

```rust
pub struct AppState {
    pub composite_payments_service: CompositePaymentsService,
    pub payments_service: crate::server::payments::Payments,
    pub refunds_service: crate::server::refunds::Refunds,
    pub disputes_service: crate::server::disputes::Disputes,
    // … 13 services owned by value
}
```

[`crates/grpc-server/grpc-server/src/http/state.rs:25`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/grpc-server/grpc-server/src/http/state.rs#L25)

The clearest illustration of the difference is one function that does both at once:

```rust
fn layer(&self, inner: S) -> Self::Service {
    TonicRequestExtensionsMiddleware {
        inner,                                  // composition — owned outright
        base_config: self.base_config.clone(),  // aggregation — just a refcount bump
    }
}
```

[`crates/grpc-server/grpc-server/src/config_overrides.rs:27`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/grpc-server/grpc-server/src/config_overrides.rs#L27)

`inner` lives and dies with the middleware. `base_config` doesn't — `.clone()` on an `Arc` copies a pointer, not a `Config`.

## Dependency

The dashed arrow in UML: used for the duration of a call, never stored.

```rust
pub fn convert_amount<T>(
    amount_convertor: &dyn AmountConvertor<Output = T>,
    amount: MinorUnit,
    currency: common_enums::Currency,
) -> Result<T, Report<errors::IntegrationError>> {
    amount_convertor.convert(amount, currency).change_context(…)
}
```

[`crates/types-traits/domain_types/src/utils.rs:309`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/domain_types/src/utils.rs#L309)

Contrast that with the connector struct further up, which *holds* a `dyn AmountConvertor` as a field. Same trait, different relationship: one borrows it, the other keeps it.

## Realisation

A connector declaring conformance — twenty-odd empty impl blocks in a row:

```rust
impl connector_types::ConnectorServiceTrait<T> for Adyen<T> {}
impl connector_types::PaymentAuthorizeV2<T> for Adyen<T> {}
impl connector_types::PaymentSyncV2 for Adyen<T> {}
impl connector_types::RefundV2 for Adyen<T> {}
// … ~20 impls
```

[`crates/integrations/connector-integration/src/connectors/adyen.rs:93`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/connectors/adyen.rs#L93)

---

# UML

## Class diagram

```
                    ┌──────────────────────────┐
                    │   ConnectorCommon        │  id(), base_url()
                    └────────────▲─────────────┘
                                 │
              ┌──────────────────┴───────────────────┐
              │  ConnectorIntegrationV2<F,C,Rq,Rs>   │  get_url(), build_request_v2()
              └──────────────────▲───────────────────┘
                                 │ pinned per flow
        ┌────────────────┬───────┴────────┬─────────────────┐
┌───────┴────────┐ ┌─────┴───────┐ ┌──────┴───────┐ ┌───────┴────────┐
│PaymentAuthorize│ │PaymentSyncV2│ │  RefundV2    │ │ PaymentCapture │  … 53 traits
│     V2<T>      │ │             │ │              │ │                │
└───────┬────────┘ └─────┬───────┘ └──────┬───────┘ └───────┬────────┘
        └────────────────┴───────┬────────┴─────────────────┘
                                 │ ~30 supertraits
                    ┌────────────┴─────────────┐
                    │  ConnectorServiceTrait<T>│   (empty body)
                    └────────────▲─────────────┘
                                 │ realises
                    ┌────────────┴─────────────┐
                    │   Elavon │ Adyen │ … 97  │
                    └──────────────────────────┘
```

## Sequence diagram

```
Client    gRPC server        ConnectorData      Connector          PSP
  │            │                   │                │               │
  ├─authorize()►                   │                │               │
  │            ├─get_connector_by_name()►           │               │
  │            │                   ├─Box<dyn ConnectorServiceTrait>►│
  │            │                   │                │               │
  │            ├─execute_connector_processing_step()►               │
  │            │                   │       ├─build_request_v2()     │
  │            │                   │       ├────── HTTP POST ──────►│
  │            │                   │       │◄───── response ────────┤
  │            │                   │       ├─handle_response_v2()   │
  │◄──proto────┤                   │                │               │
```

## State machine

`AttemptStatus` has 31 variants. The states and their terminality are encoded in the type; the edges are the conventional lifecycle, driven by each connector's status mapping rather than declared in one place.

```
      Started
         │
         ▼
  AuthenticationPending ─────► AuthenticationFailed ●
         │
         ▼
  AuthenticationSuccessful
         │
         ▼
     Authorizing ────────────► AuthorizationFailed ●
         │
         ▼
     Authorized ──► VoidInitiated ──► Voided ●
         │                        └─► VoidFailed ●
         ▼
  CaptureInitiated ───────────► CaptureFailed ●
         │
         ▼
      Charged ● ──► AutoRefunded ●

  ● = terminal, per AttemptStatus::is_terminal_status (enums.rs:1426)
```

For the use-case and activity views of the payment lifecycle, see [Services and Methods](../architecture/concepts/services-and-methods.md).

---

# Design Patterns

Each of these showed up because a specific pressure produced it, not because it was on a checklist — Bridge exists because 97 × 30 is a real number.

## Factory Method

Turns a runtime connector name into a live implementation. This is the seam where static types become dynamic dispatch — and it's polymorphism doing the actual work:

```rust
pub fn get_connector_by_name(connector_name: &ConnectorEnum) -> Self {
    let connector = Self::convert_connector(*connector_name);
    Self { connector, connector_name: *connector_name }
}

fn convert_connector(connector_name: ConnectorEnum) -> BoxedConnector<T> {
    match connector_name {
        ConnectorEnum::Forte => Box::new(connectors::Forte::new()),
        ConnectorEnum::Adyen => Box::new(connectors::Adyen::new()),
        ConnectorEnum::Razorpay => Box::new(connectors::Razorpay::new()),
        // … all 97 variants, exhaustive — no wildcard arm
    }
}
```

[`crates/integrations/connector-integration/src/types.rs:30`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/types.rs#L30)

An enum goes in, a `Box<&'static dyn ConnectorServiceTrait<T>>` comes out, and everything downstream sees only the trait object.

The match is **exhaustive**, with no `_` arm. Add a connector to the enum and forget to register it, and the compiler stops you. The registry can't silently rot.

## Abstract Factory

Factory Method gets you one connector. Abstract Factory gets you *families*. Payments, surcharge, FRM, and payouts each have their own enum, product type, and factory:

```rust
impl SurchargeConnectorData {
    fn convert_connector(connector_name: SurchargeConnectorEnum) -> BoxedSurchargeConnector {
        match connector_name {
            SurchargeConnectorEnum::Interpayments => Box::new(surcharge_connectors::InterPayments::new()),
        }
    }
}

impl PayoutConnectorData {
    fn convert_connector(connector_name: PayoutConnectorEnum) -> BoxedPayoutConnector {
        match connector_name {
            PayoutConnectorEnum::Loonio => Box::new(payout_connectors::LoonioPayouts::new()),
            PayoutConnectorEnum::Paypal => Box::new(payout_connectors::PaypalPayouts::new()),
            PayoutConnectorEnum::Cybersource => Box::new(payout_connectors::CybersourcePayouts::new()),
            // …
        }
    }
}
```

[`crates/integrations/connector-integration/src/types.rs:150`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/types.rs#L150) and [`:208`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/types.rs#L208)

What makes it Abstract Factory rather than four unrelated functions is the trait tying them together:

```rust
pub trait ConnectorDataProvider: Sized {
    type ConnectorEnumType: Copy;

    fn from_connector_variant(variant: &ConnectorVariant) -> Option<Self>;
}
```

[`crates/integrations/connector-integration/src/types.rs:224`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/types.rs#L224)

Callers ask for a family without knowing which they'll get, and a payout connector can't be mistaken for a payment connector.

## Singleton

Every connector's struct and constructor are generated by one macro, `create_all_prerequisites!` — that's why the snippet below is full of `$` metavariables. The generated `new()` doesn't allocate. It's a `const fn` returning `&'static Self`:

```rust
pub const fn new() -> &'static Self {
    &Self {
        $($converter_name: &common_utils::types::[<$amount_unit ForConnector>],)*
        $([<$flow_name:snake>]: &Bridge::<…>(PhantomData),)*
        _marker: std::marker::PhantomData,
    }
}
```

[`crates/integrations/connector-integration/src/connectors/macros.rs:1173`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/connectors/macros.rs#L1173)

Rust const-promotes that into static memory, so all 97 connectors are compile-time singletons. `Adyen::new()` hands back the same object every time, forever, for free — Singleton without the usual misery: no lock, no lazy init, no global mutable state. It's also why `BoxedConnector<T>` is `Box<&'static dyn …>`: the box wraps a reference, not the connector.

## Builder

`Request` has seven fields, most optional, so construction is separated from representation:

```rust
impl RequestBuilder {
    pub fn new() -> Self { … }

    pub fn url(mut self, url: &str) -> Self { self.url = url.into(); self }
    pub fn method(mut self, method: Method) -> Self { self.method = method; self }

    pub fn attach_default_headers(mut self) -> Self {
        self.headers.extend(default_request_headers());
        self
    }

    pub fn build(self) -> Request { … }
}
```

[`crates/common/common_utils/src/request.rs:278`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/common/common_utils/src/request.rs#L278)

Each setter takes `mut self` and returns `Self`, so calls chain and the value is consumed at `build()`.

## Adapter

Every processor's vocabulary has to be translated, and this is where all of it goes:

```rust
impl From<GlobalpayPaymentStatus> for AttemptStatus {
    fn from(status: GlobalpayPaymentStatus) -> Self {
        match status {
            GlobalpayPaymentStatus::Captured => Self::Charged,
            GlobalpayPaymentStatus::Preauthorized => Self::Authorized,
            GlobalpayPaymentStatus::Declined => Self::Failure,
            // …
            GlobalpayPaymentStatus::Reversed => Self::Voided,
        }
    }
}
```

[`crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs:135`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/connectors/globalpay/transformers.rs#L135)

`TryFrom` is Rust's idiomatic spelling of Adapter, and it's the largest thing in the codebase: **96 transformer files, hundreds of impls**. That's not accidental. The mess is real and irreducible — processors genuinely disagree — so the design gives it exactly one place to live.

## Bridge

97 connectors × ~30 flows is 2,900 request/response pairings. Write those by hand and you're done for.

```rust
pub trait BridgeRequestResponse: Send + Sync {
    type RequestBody;
    type ResponseBody;
    type ConnectorInputData: FlowTypes;

    fn request_body(&self, rd: Self::ConnectorInputData) -> CustomResult<Self::RequestBody, IntegrationError>
    where Self::RequestBody: TryFrom<Self::ConnectorInputData, Error = Report<IntegrationError>>
    {
        Self::RequestBody::try_from(rd)
    }
}

pub struct Bridge<Q, S, T>(pub PhantomData<(Q, S, T)>);
```

[`crates/integrations/connector-integration/src/connectors/macros.rs:107`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/integrations/connector-integration/src/connectors/macros.rs#L107)

We named the pattern in the source, because that's what it is. The flow hierarchy varies on one axis, the connector's wire types on the other, and `Bridge` binds them with associated types instead of code. It's zero-sized — it costs nothing at runtime. Note where it lands: `TryFrom`. Bridge hands off to Adapter.

## Facade

Thirty flow traits is a lot to ask a caller to know. One trait fronts them all — `ConnectorServiceTrait`, shown under Inheritance, whose empty body adds no behaviour and exists purely to give the system one name to depend on.

The gRPC layer is the second facade. Each handler hides metadata extraction, config resolution, proto conversion, connector construction, and flow execution behind one RPC:

```rust
#[tonic::async_trait]
impl PaymentService for Payments {
    async fn authorize(
        &self,
        request: tonic::Request<PaymentServiceAuthorizeRequest>,
    ) -> Result<tonic::Response<PaymentServiceAuthorizeResponse>, tonic::Status> {
        info!("PAYMENT_AUTHORIZE_FLOW: initiated");
        // …
    }
}
```

[`crates/grpc-server/grpc-server/src/server/payments.rs:856`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/grpc-server/grpc-server/src/server/payments.rs#L856)

## Decorator

Middleware wraps a service, implements the same trait as the thing it wraps, adds behaviour, then delegates:

```rust
pub struct TonicRequestExtensionsMiddleware<S> {
    inner: S,
    base_config: Arc<Config>,
}

impl<S> Service<Request<Body>> for TonicRequestExtensionsMiddleware<S> {
    fn call(&mut self, mut req: Request<Body>) -> Self::Future {
        let config_override = req.headers().get("x-config-override").and_then(|h| h.to_str().ok());

        match extract_and_merge_config(config_override, &self.base_config) {
            Ok(cfg) => { req.extensions_mut().insert(cfg); }
            Err(e) => {
                let err = e.into_grpc_status();
                return Box::pin(async move { Err(err) });   // ← never calls inner
            }
        }

        let future = self.inner.call(req);                  // ← delegate
        Box::pin(async move { Ok(future.await?) })
    }
}
```

[`crates/grpc-server/grpc-server/src/config_overrides.rs:35`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/grpc-server/grpc-server/src/config_overrides.rs#L35)

Because `Middleware<S>` implements `Service` and holds an `S: Service`, decorators stack arbitrarily without anything knowing how deep it goes.

## Chain of Responsibility

That's the same code read differently. The `Err` branch above returns without ever touching `inner` — a link that can handle-and-stop. The chain itself:

```rust
Server::builder()
    .layer(logging_layer)
    .layer(request_id_layer)
    .layer(propagate_request_id_layer)
    .layer(config_override_layer)
    .layer(metrics_layer)
    .add_service(reflection_service)
```

[`crates/grpc-server/grpc-server/src/app.rs:318`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/grpc-server/grpc-server/src/app.rs#L318)

A bad `x-config-override` header dies at layer four and never reaches the payment logic. Tower is deliberately both patterns at once; it's worth naming both rather than picking a side.

## Flyweight

Building an HTTP client is expensive, and many merchants share proxy settings, so clients are pooled by config:

```rust
static PROXY_CLIENT_CACHE: OnceCell<RwLock<HashMap<(Proxy, String), Client>>> = OnceCell::new();

fn get_or_create_proxy_client(
    cache: &RwLock<HashMap<(Proxy, String), Client>>,
    cache_key: (Proxy, String),
    …
) -> CustomResult<Client, ApiClientError> {
    let read_result = cache.read().ok().and_then(|lock| lock.get(&cache_key).cloned());

    let client = match read_result {
        Some(cached_client) => cached_client,
        None => {
            let mut write_lock = cache.try_write().map_err(|_| ApiClientError::ClientConstructionFailed)?;
            match write_lock.get(&cache_key) {     // ← double-checked: someone may have won the race
                Some(cached_client) => cached_client.clone(),
                None => { /* construct and insert */ }
            }
        }
    };
    …
}
```

[`crates/common/external-services/src/service.rs:1309`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/common/external-services/src/service.rs#L1309)

The proxy config is the intrinsic state and the pool is keyed on it. Note the second lookup after taking the write lock — classic double-checked locking, because another thread may have built the client while we waited.

## Strategy

The connector *is* the algorithm. This blanket impl is what makes any of them interchangeable:

```rust
impl<S, Flow, ResourceCommonData, Req, Resp>
    ConnectorIntegrationAnyV2<Flow, ResourceCommonData, Req, Resp> for S
where
    S: ConnectorIntegrationV2<Flow, ResourceCommonData, Req, Resp> + Send + Sync,
{
    fn get_connector_integration_v2(&self) -> BoxedConnectorIntegrationV2<'_, …> {
        Box::new(self)
    }
}
```

[`crates/types-traits/interfaces/src/connector_integration_v2.rs:32`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/connector_integration_v2.rs#L32)

Implement the trait and you get type erasure for free — no registration, no boilerplate. There's a smaller, more textbook Strategy in the crypto layer, where a connector says which signature algorithm it uses and the verifier consumes it blind:

```rust
fn get_algorithm(&self) -> CustomResult<Box<dyn crypto::VerifySignature + Send>, IntegrationError> {
    Ok(Box::new(crypto::NoAlgorithm))
}
```

[`crates/types-traits/interfaces/src/verification.rs:29`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/verification.rs#L29) — with `HmacSha256`, `Ed25519`, and `Blake3` as alternatives.

## Template Method

`build_request_v2` is the invariant skeleton. `get_url` is the only required hook; everything else has a default:

```rust
fn build_request_v2(&self, req: &RouterDataV2<…>) -> CustomResult<Option<Request>, IntegrationError> {
    Ok(Some(
        RequestBuilder::new()
            .method(self.get_http_method())
            .url(self.get_url(req)?.as_str())
            .attach_default_headers()
            .headers(self.get_headers(req)?)
            .set_optional_body(self.get_request_body(req)?)
            .build(),
    ))
}
```

[`crates/types-traits/interfaces/src/connector_integration_v2.rs:127`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/connector_integration_v2.rs#L127)

The algorithm is fixed; the steps are overridable. A connector never writes request-assembly logic — it answers questions about itself and the skeleton assembles the result. The body is also the Builder chain, so both patterns meet here. A second, outer Template Method at `service.rs:538` fixes the order build → call → handle for every flow and every connector.

## Null Object

The `NoAlgorithm` default above is a pattern in its own right:

```rust
impl VerifySignature for NoAlgorithm {
    fn verify_signature(&self, _secret: &[u8], _signature: &[u8], _msg: &[u8])
        -> CustomResult<bool, errors::CryptoError>
    {
        Ok(true)
    }
}
```

[`crates/common/common_utils/src/crypto.rs:129`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/common/common_utils/src/crypto.rs#L129)

A connector with no webhook signing doesn't hand back a `None` that every caller must unwrap. It hands back an object that satisfies the interface and does nothing. No branch at the call site.

## State

The payment lifecycle is a state machine, modelled as an enum with 31 states and exhaustive transitions (the diagram is above). Modelling states as data rather than polymorphic objects buys something specific here: every mapping from a processor's status into `AttemptStatus` is a total `match`, so adding a state makes the compiler point at every site that now has to handle it.

## Observer

Webhooks are an observer relationship — the processor is the subject, Prism is the observer, and this is the callback interface:

```rust
pub trait IncomingWebhook {
    fn verify_webhook_source(
        &self,
        _request: RequestDetails,
        _connector_webhook_secret: Option<ConnectorWebhookSecrets>,
        _connector_account_details: Option<ConnectorSpecificConfig>,
    ) -> Result<bool, error_stack::Report<WebhookError>> {
        Ok(false)
    }
}
```

[`crates/types-traits/interfaces/src/connector_types.rs:516`](https://github.com/juspay/hyperswitch-prism/blob/b406d5ee29e92680c85a4417a8e0b69e5a51d10b/crates/types-traits/interfaces/src/connector_types.rs#L516)

Every method has a safe default, so a connector that doesn't sign its webhooks inherits sensible behaviour instead of implementing empty methods.

---

# What this actually means

Strip away the vocabulary and it's four ideas that compose:

```
create_all_prerequisites!  ──generates──►  connector struct
                                           const fn new() -> &'static Self      [Singleton]
                                                 │ holds one Bridge per flow
                                                 ▼
ConnectorEnum::Adyen ──get_connector_by_name──► Box<dyn ConnectorServiceTrait>  [Factory → Strategy]
                                                 │
                                                 ▼
                          execute_connector_processing_step                     [Template Method]
                             build_request_v2 → HTTP → handle_response_v2
                                                 │
                                                 ▼
                                TryFrom<RouterData> for AdyenPaymentRequest     [Adapter]
```

A macro builds a singleton holding bridges. A factory hands it over as a strategy. A template method drives it. An adapter translates at the edge. Everything else in this post is a consequence of those four.

None of these ideas are new. Encapsulation and the GoF patterns are older than most of the processors we integrate with. What's changed is the pressure: at two connectors you can skip every one of them and ship. At 97 the design either holds or you feel it in every pull request.

**If you want to add a connector:** you'll write two files — `<name>.rs` for wiring, `<name>/transformers.rs` for mapping. The connector's HTTP quirks live in one, its wire format in the other, and nothing about it leaks anywhere else. The compiler will tell you if you've missed a registration, because the factory match is exhaustive.

**If you're deciding whether this stuff is worth it:** that's the shape of the answer. Not that the abstractions are elegant — that a new processor lands as two new files instead of 97 edits.

---

**Reference:** [Hyperswitch Prism on GitHub](https://github.com/juspay/hyperswitch-prism)
