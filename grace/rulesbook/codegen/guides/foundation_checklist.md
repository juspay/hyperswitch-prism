# Foundation Checklist — Post `add_connector.sh` Steps

**MANDATORY**: Every Foundation Setup Subagent MUST complete this step after
`add_connector.sh` succeeds and before moving to flow implementation.
Skipping it will produce a connector that compiles but silently never invokes
pre-auth flows at runtime.

---

## `ValidationTrait` overrides (fixes silent flow-never-invoked)

**Why:** `connector_types::ValidationTrait` has five `should_do_*` methods that
all default to `false`. If you implement a pre-auth flow (CreateOrder, CreateSessionToken,
CreateAccessToken, PaymentMethodToken) but do NOT override the corresponding method to
return `true`, the flow is compiled in but **never invoked at runtime**. No error —
the flow is simply skipped.

**Rule:** Add a `ValidationTrait` override **only** if your connector implements the
corresponding pre-auth flow (i.e., the flow appears in your `create_all_prerequisites!`
macro). If your connector does not use a pre-auth flow, do not add the override — the
default `false` is correct. If your connector implements any of the flows below, you
MUST add the corresponding override or the flow will be skipped at runtime:

### `should_do_order_create` — required for `CreateOrder` flow

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for YourNewConnector<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }
}
```

Also add the empty trait marker impl:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentOrderCreate for YourNewConnector<T>
{}
```

Real examples: `Razorpay`, `Cashfree`, `Payme`, `Paytm`, `Revolut`

### `should_do_session_token` — required for `CreateSessionToken` flow

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for YourNewConnector<T>
{
    fn should_do_session_token(&self) -> bool {
        true
    }
}
```

Also add the empty trait marker impl for `CreateSessionToken`:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentSessionToken for YourNewConnector<T>
{}
```

> **Note:** `SdkSessionTokenV2` is a separate SDK-session flow (distinct from `CreateSessionToken`).
> Add it only if your connector also implements `SdkSessionToken`:
>
> ```rust
> impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
>     connector_types::SdkSessionTokenV2 for YourNewConnector<T>
> {}
> ```
>
> Real connectors that implement both `SdkSessionToken` and `CreateSessionToken`: `Nuvei`, `Paytm`

### `should_do_access_token` — required for `CreateAccessToken` flow

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for YourNewConnector<T>
{
    fn should_do_access_token(
        &self,
        _payment_method: Option<common_enums::PaymentMethod>,
    ) -> bool {
        true
    }
}
```

Also add the empty trait marker impl:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentAccessToken for YourNewConnector<T>
{}
```

Real examples: `Paypal`, `TrueLayer`, `JPMorgan`, `Getnet`, `Volt`, `Globalpay`, `Airwallex`

### `should_do_payment_method_token` — required for `PaymentMethodToken` flow

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for YourNewConnector<T>
{
    fn should_do_payment_method_token(
        &self,
        payment_method: common_enums::PaymentMethod,
        _payment_method_type: Option<common_enums::PaymentMethodType>,
    ) -> bool {
        // Return true only for payment methods your connector tokenizes
        matches!(
            payment_method,
            common_enums::PaymentMethod::Card
        )
    }
}
```

Also add the empty trait marker impl:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::PaymentTokenV2<T> for YourNewConnector<T>
{}
```

Real examples: `Stax`, `Braintree`, `Paysafe`, `Finix`, `Mollie`, `Hipay`

### Combining multiple overrides

If your connector needs more than one pre-auth flow, combine them in a single impl block:

```rust
impl<T: PaymentMethodDataTypes + std::fmt::Debug + Sync + Send + 'static + Serialize>
    connector_types::ValidationTrait for YourNewConnector<T>
{
    fn should_do_order_create(&self) -> bool {
        true
    }

    fn should_do_session_token(&self) -> bool {
        true
    }
}
```

Real example: `Paytm` overrides both `should_do_session_token` and `should_do_order_create`.
