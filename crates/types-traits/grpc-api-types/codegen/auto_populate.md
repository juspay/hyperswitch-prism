# Auto Populate Codegen Design

This document explains the decisions behind `auto_populate.rs`, which generates typed request-population traits and service wrappers from the compiled protobuf descriptor set.

## Goal

Some sanity-layer values are known at runtime, but only some protobuf request types have the destination field. Application code should be able to say “populate this field if this request supports it” without maintaining a manual list of request structs.

The protobuf schema is the source of truth:

- If a request message contains the configured field, generated code should populate it.
- If a request message does not contain the field, generated code should no-op.
- If a new RPC method uses a request containing the field, rebuilding should pick it up automatically.
- Business code should not know which request/message types support the field.

## Why Use `FileDescriptorSet`

`prost-build` already produces a `FileDescriptorSet` during protobuf compilation. That descriptor contains:

- protobuf packages
- services
- RPC methods
- method input/output message types
- message fields
- field shapes, including `optional`, `repeated`, message type, and oneof metadata

Using this descriptor avoids runtime protobuf reflection and avoids hand-maintained mappings such as:

```rust
os_based_return_url => [
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest,
    AuthenticatorClientAuthenticationContext,
]
```

The generator reads the descriptor at build time and emits normal Rust code into `OUT_DIR`.

## Why Limit To The `types` Package

The generator intentionally processes only the protobuf package named `types`.

Reasoning:

- UCS request/response structs are generated under `crate::payments`.
- Other descriptor packages, such as health/reflection, are not request-domain types.
- Generating implementations for external packages would create invalid Rust paths or unnecessary trait impls.

This is controlled by:

```rust
const SUPPORTED_PACKAGE: &str = "types";
const SUPPORTED_PACKAGE_MODULE: &str = "crate::payments";
```

If another generated module/package needs this mechanism later, the generator should be extended deliberately instead of assuming all descriptor packages map to `crate::payments`.

## Developer-Written Contract: `AutoPopulateField`

`AutoPopulateField` is the only hand-written field registry. Each enum variant
implements its codegen behavior through regular enum methods.

For each field, the developer writes:

- protobuf field name
- generated trait name
- generated method name
- Rust value type
- setter logic for optional fields
- setter logic for non-optional singular fields

Example:

```rust
#[derive(Clone, Copy)]
enum AutoPopulateField {
    OsBasedReturnUrl,
}

const AUTO_POPULATE_FIELDS: &[AutoPopulateField] = &[AutoPopulateField::OsBasedReturnUrl];

impl AutoPopulateField {
    fn field_name(self) -> &'static str {
        match self {
            Self::OsBasedReturnUrl => "os_based_return_url",
        }
    }

    fn trait_name(self) -> &'static str {
        match self {
            Self::OsBasedReturnUrl => "PopulateOsBasedReturnUrl",
        }
    }

    fn method_name(self) -> &'static str {
        match self {
            Self::OsBasedReturnUrl => "populate_os_based_return_url",
        }
    }

    fn value_type(self) -> TokenStream {
        match self {
            Self::OsBasedReturnUrl => quote! { crate::payments::OsBasedReturnUrl },
        }
    }

    fn setter_body(self, shape: FieldShape, message_name: &str) -> TokenStream {
        match self {
            Self::OsBasedReturnUrl => match shape {
                FieldShape::Optional => quote! {
                    if let Some(existing) = self.os_based_return_url.as_mut() {
                        if existing.os_type != 0 {
                            existing.return_url_map = value.return_url_map;
                        }
                    }
                },
                FieldShape::Required => quote! {
                    if self.os_based_return_url.os_type != 0 {
                        self.os_based_return_url.return_url_map = value.return_url_map;
                    }
                },
                FieldShape::Repeated => {
                    let error = format!(
                        "populate_os_based_return_url: `{message_name}.os_based_return_url` is repeated"
                    );
                    quote! {
                        let _ = value;
                        compile_error!(#error);
                    }
                },
            },
        }
    }
}
```

The developer does not list request/message types. The descriptor decides where the field exists.

The generator uses `quote`/`proc_macro2` to build Rust tokens and
`prettyplease` to format the generated file. That avoids string-built Rust
setter bodies while keeping this as ordinary build-script code, not a separate
procedural macro crate.

## Why Setter Bodies Are Shape-Based

Different proto field shapes generate different Rust field types:

- `optional SomeMessage field = 1;` becomes `Option<SomeMessage>`
- `SomeMessage field = 1;` **also** becomes `Option<SomeMessage>` — prost always
  wraps a singular message-typed field in `Option`, `optional` keyword or not,
  because message fields track presence independently of any scalar default.
  `field_shape()` accounts for this: any message-typed field is classified as
  `FieldShape::Optional`, regardless of `proto3_optional`.
- `optional int32 field = 1;` becomes `Option<i32>`; `int32 field = 1;`
  (no `optional`) becomes plain `i32` — this is where `FieldShape::Required`
  actually applies. It is only correct for scalar fields.
- `repeated SomeMessage field = 1;` becomes `Vec<SomeMessage>`

So the setter cannot be one generic assignment for every shape.

The current contract supports these `FieldShape` variants in `setter_body`:

- `FieldShape::Optional` — used for every `optional` field, and for every
  message-typed field even without `optional`
- `FieldShape::Required` — scalar fields only; never reachable for a
  message-typed field

`repeated` is intentionally rejected with a generated `compile_error!`. This is safer than silently picking behavior for a shape that has not been designed.

## Current `os_based_return_url` Semantics

`os_based_return_url` is optional on the request context, but if it is present, `os_type` must be a concrete `ClientPlatform` in practice.

The generated setter follows that rule:

- It does not create `os_based_return_url` if the request did not send it.
- It preserves the request-provided `os_type`.
- It only fills `return_url_map` when `os_type` is not `CLIENT_PLATFORM_UNSPECIFIED`.

This keeps responsibility split cleanly:

- caller/request provides the selected OS
- sanity layer provides the configured URL map
- downstream logic can choose from the map using the request OS

## Connector Sanity Dispatch

The runtime sanity layer does not dispatch by manually scanning connector
headers as strings. It reuses UCS's typed connector metadata parser:

```rust
ucs_interface_common::metadata::connector_variant_from_metadata
```

That produces `ConnectorVariant`, so the dispatcher can match connector
families explicitly:

```rust
ConnectorVariant::Authenticator(AuthenticatorConnectorEnum::Plaid)
```

This keeps the normal path aligned with the rest of UCS. If connector metadata
alone is not enough, the runtime layer uses the existing
`connector_and_config_from_metadata` helper to derive the connector from
`x-connector-config` instead of maintaining its own string-to-connector mapping.
The Plaid sanity implementation still reads raw JSON to extract the Euler-only
extra fields, because those fields are intentionally not part of UCS's typed
`PlaidConfig`.

## Request Discovery

`request_message_names` walks:

```text
FileDescriptorSet
  -> file[]
  -> service[]
  -> method[]
  -> input_type
```

This means “request type” is defined as “a message used as an RPC input type”, not by naming convention.

This avoids fragile assumptions like:

- message name ends with `Request`
- message belongs to a specific file
- service is manually listed

If a new method is added to an existing service, the generator sees the new method from the descriptor and emits wrapper code for it.

## Message Field Discovery

`message_index` builds a map:

```text
top-level message name -> DescriptorProto
```

Then the generator checks whether each message either:

- directly contains the configured field, or
- can reach the configured field through nested message fields

This is needed for request shapes like:

```proto
message MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
  oneof domain_context {
    AuthenticatorClientAuthenticationContext authenticator = 8;
  }
}

message AuthenticatorClientAuthenticationContext {
  optional OsBasedReturnUrl os_based_return_url = 8;
}
```

The top-level request does not directly contain `os_based_return_url`, but it can reach it through the `authenticator` oneof variant.

## Why Generate No-Ops

Every request message gets an implementation for the configured populate trait.

If the field is not present and cannot be reached through nested messages, the implementation is:

```rust
let _ = value;
```

Reasoning:

- generic business/sanity code can call the populate method on any request type
- unsupported request types naturally do nothing
- the caller does not branch on request type
- adding a new request type does not require implementing a trait manually

## Nested Message Delegation

If a top-level request does not directly contain the field but has a nested message that can contain it, generated code delegates to the nested message.

For normal optional message fields, the generated shape is:

```rust
if let Some(inner) = self.some_context.as_mut() {
    inner.populate_os_based_return_url(value);
} else {
    let _ = value;
}
```

If the nested message is absent, the setter no-ops. The generator does not create missing nested contexts because that would require business rules the schema alone cannot provide.

## Oneof Delegation

For oneof fields, the generator emits a `match` over every oneof variant that can reach the target field.

This matters because more than one oneof variant may contain similarly named fields. The generator must not select only the first matching variant.

For `os_based_return_url`, only the `Authenticator` variant currently matches. The generated logic is equivalent to:

```rust
match self.domain_context.as_mut() {
    Some(DomainContext::Authenticator(inner)) => {
        inner.populate_os_based_return_url(value);
    }
    _ => {
        let _ = value;
    }
}
```

This preserves the oneof selected by the caller and only mutates the active variant.

## Service Wrapper Generation

The generator also emits a generic `SanityLayer<S, Z>` wrapper.

For each supported tonic service, it generates:

```rust
impl<S, Z> SomeService for SanityLayer<S, Z>
where
    S: SomeService,
    Z: RequestSanitizer,
{
    async fn some_method(&self, mut request: tonic::Request<RequestType>) -> Result<...> {
        let metadata = request.metadata().clone();
        self.sanitizer.sanitize(&metadata, request.get_mut());
        self.inner.some_method(request).await
    }
}
```

Reasoning:

- sanity runs before the real handler
- no flow handler file needs to be modified
- HTTP handlers that call the same service object also pass through the wrapper
- metadata can be inspected without being rewritten
- the typed request body can be mutated before business logic sees it

## Why Runtime Use Is Feature-Gated

The generated wrapper exists so the sanity layer can run before every typed RPC handler. That is useful for Euler/Plaid compatibility, but it should not affect the normal UCS path unless explicitly enabled.

`grpc-server` therefore uses a Cargo feature:

```bash
cargo run -p grpc-server --features connector-sanity-layer
```

When `connector-sanity-layer` is enabled:

- `crate::sanity_layer::wrap(service)` returns the generated `SanityLayer`
- every wrapped service method calls the sanitizer before the real handler
- Plaid redirect keys can be read from raw metadata and copied into the typed request

When `connector-sanity-layer` is disabled:

- `crate::sanity_layer::wrap(service)` returns the original service
- there is no generated wrapper in the runtime call path
- normal UCS behavior is unchanged

This keeps integration code stable while avoiding runtime overhead for builds that do not need the compatibility layer.

## Runtime Responsibility

The generated code does not know Plaid, Euler, or merchant-specific config structure.

Runtime connector-specific code lives with the connector. The top-level
`grpc-server/src/sanity_layer.rs` owns wrapping, connector resolution, raw
metadata extraction, and dispatch. Plaid-specific behavior lives in
`connector-integration/src/authenticator_connectors/plaid/sanity.rs`.

Connector-specific behavior is exposed through the object-safe
`ConnectorSanity` trait. `ConnectorSanityExt::sanity()` converts the resolved
`ConnectorVariant` into `&'static dyn ConnectorSanity`. The generic layer
extracts raw config using `ConnectorVariant::get_connector_name()` and calls
`apply`. Each connector sanity implementation owns:

- how to apply its sanity behavior to a typed request

Connector handlers receive `&mut dyn ConnectorSanityRequest`, an object-safe
request interface that exposes the generated populate operations needed by
sanity code. The generated trait names stay behind this boundary; connector
logic calls methods such as `req.populate_os_based_return_url(value)`.

For Plaid, the generic layer:

- resolves a connector name from metadata or raw config
- calls `connector.sanity()` to get the registered sanitizer
- extracts that connector's raw config object from `x-connector-config`

Then Plaid's connector-owned sanity handler:

- reads Euler-only keys from the raw Plaid config
- builds an `OsBasedReturnUrl` value containing `return_url_map`
- calls `populate_os_based_return_url`

This keeps generated infrastructure generic and connector-specific policy outside `grpc-api-types`.

Unsupported connectors resolve to `NOOP_SANITY`, whose default `apply` logs
that no connector sanity is registered and performs no mutation. That makes the
runtime structure a connector sanity registry instead of a Plaid-specific
predicate.

## Adding A New Auto-Populated Field

To add another field to the generated sanity infrastructure:

1. Add the destination field to the relevant `.proto` message or messages.
2. Add a new variant to `AutoPopulateField`.
3. Add that variant to `AUTO_POPULATE_FIELDS`.
4. Add match arms for that variant in `impl AutoPopulateField`:
   - `field_name`: exact proto field name
   - `trait_name`: generated Rust trait name
   - `method_name`: generated Rust method name
   - `value_type`: Rust type accepted by the generated setter
   - `setter_body`: shape-based setter logic using `quote!`
5. Add or update focused tests in `grpc-api-types/tests/auto_populate_test.rs`.
6. Build or test `grpc-api-types`; the generated file in `OUT_DIR` will include
   trait impls for every descriptor-discovered matching message.

Example shape:

```rust
impl AutoPopulateField {
    fn field_name(self) -> &'static str {
        match self {
            Self::NativeAppIdentifier => "native_app_identifier",
            // existing variants...
        }
    }

    fn trait_name(self) -> &'static str {
        match self {
            Self::NativeAppIdentifier => "PopulateNativeAppIdentifier",
            // existing variants...
        }
    }

    fn method_name(self) -> &'static str {
        match self {
            Self::NativeAppIdentifier => "populate_native_app_identifier",
            // existing variants...
        }
    }

    fn value_type(self) -> TokenStream {
        match self {
            Self::NativeAppIdentifier => quote! { String },
            // existing variants...
        }
    }

    fn setter_body(self, shape: FieldShape, message_name: &str) -> TokenStream {
        match self {
            Self::NativeAppIdentifier => match shape {
                FieldShape::Optional => quote! {
                    self.native_app_identifier = Some(value);
                },
                FieldShape::Required => quote! {
                    self.native_app_identifier = value;
                },
                FieldShape::Repeated => {
                    let error = format!(
                        "populate_native_app_identifier: `{message_name}.native_app_identifier` is repeated"
                    );
                    quote! {
                        let _ = value;
                        compile_error!(#error);
                    }
                },
            },
            // existing variants...
        }
    }
}
```

No request/message list is needed. The descriptor decides which generated proto
types get a real setter, a nested delegate, or a no-op.

If runtime code needs to call more than one generated populate method, update
the sanitizer bound in `grpc-server/src/sanity_layer.rs`. For example:

```rust
fn sanitize<T: PopulateOsBasedReturnUrl + PopulateNativeAppIdentifier>(
    &self,
    metadata: &MetadataMap,
    req: &mut T,
)
```

The generated `RequestSanitizer` trait already includes every auto-populate
field from `AUTO_POPULATE_FIELDS`, so connector runtime code must use matching
bounds when it exposes those methods through `ConnectorSanityRequest`.

## Adding A New Connector Sanity Handler

Connector-specific sanity logic lives in
the connector's own module, not in generated code. For Plaid this is
`crates/integrations/connector-integration/src/authenticator_connectors/plaid/sanity.rs`.

To add a new connector:

1. Make sure the connector can be resolved to a `ConnectorVariant` by the
   existing metadata/config parsing path.
2. Add a connector-specific sanity module/struct, for example `foo_pay/sanity.rs`
   with `FooPaySanity`.
3. Implement `ConnectorSanity` for the connector sanitizer. Its `apply` method
   should accept `Option<SecretSerdeValue>` plus `&mut dyn ConnectorSanityRequest`
   and call field-specific helper methods internally.
4. Add one match arm in `ConnectorSanityExt::sanity` in
   `connector-integration/src/sanity.rs` for the connector enum variant. The
   server should keep calling `connector.sanity()`; raw config extraction and
   `apply` invocation remain generic.
5. Leave all unsupported connectors on the `NOOP_SANITY` path; it should log
   `no connector sanity registered` and perform no mutation.
6. Add a focused test or smoke command that sends raw `x-connector-config` for
   the connector and confirms the typed request is populated before the handler
   runs.

Example shape:

```rust
struct FooPaySanity;

impl ConnectorSanity for FooPaySanity {
    fn apply(&self, raw_config: Option<SecretSerdeValue>, req: &mut dyn ConnectorSanityRequest) {
        let Some(config) = raw_config else {
            return;
        };
        let Some(value) = build_value_from_foo_pay_config(config) else {
            return;
        };

        req.populate_os_based_return_url(value);
    }
}
```

Then register it by adding an arm inside the existing
`impl ConnectorSanityExt for ConnectorVariant`:

```rust
ConnectorVariant::Payment(ConnectorEnum::FooPay) => &FOO_PAY_SANITY,
```

The connector handler owns the translation from merchant/Euler-specific config
shape into UCS's generic request shape. The auto-populate generator only owns
finding destination fields and emitting typed setters.

## What Happens When New Proto Is Added

New method in an existing service:

- automatically appears in the descriptor
- generated `SanityLayer` gets a wrapper method
- request type gets populate trait impls

New request message used by an existing/new method:

- automatically appears as a method input type
- gets no-op or real setter based on its fields

New service:

- generated code can emit an implementation for it
- the server composition root still needs to wrap/register the service with `crate::sanity_layer::wrap`

This is the remaining explicit integration point because service construction/registration is application wiring, not protobuf type generation.

## Failure Modes

Unsupported repeated target field:

- generated code emits `compile_error!`
- reason: repeated-field merge semantics are business-specific

Unsupported repeated nested path:

- generated code emits `compile_error!`
- reason: choosing which element to mutate is business-specific

Missing nested context:

- generated code no-ops
- reason: creating missing domain context would require business logic

Unknown package/module:

- ignored unless it is `types`
- reason: only `types` currently maps to `crate::payments`

Malformed raw connector config at runtime:

- Plaid sanity extraction no-ops
- downstream normal validation still handles typed config errors

## Why This Is Not Runtime Reflection

The descriptor is inspected only during Rust build/codegen.

At runtime:

- the service wrapper is normal Rust code
- setters are normal trait methods
- matching oneof variants is normal enum matching
- no protobuf descriptor lookup is needed

This gives the maintainability of descriptor-driven discovery without runtime reflection cost or stringly typed request mutation.
