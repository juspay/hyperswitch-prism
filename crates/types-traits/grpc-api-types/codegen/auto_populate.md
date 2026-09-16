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

## Developer-Written Contract: `AutoPopulateSpec`

`AutoPopulateField` is the only hand-written field registry. Each enum variant
delegates to a small spec type that implements `AutoPopulateSpec`.

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

#[derive(Clone, Copy)]
struct OsBasedReturnUrlSpec;

impl AutoPopulateSpec for OsBasedReturnUrlSpec {
    fn field_name(self) -> &'static str {
        "os_based_return_url"
    }

    fn trait_name(self) -> &'static str {
        "PopulateOsBasedReturnUrl"
    }

    fn method_name(self) -> &'static str {
        "populate_os_based_return_url"
    }

    fn value_type(self) -> TokenStream {
        quote! { crate::payments::OsBasedReturnUrl }
    }

    fn setter_body(self, shape: FieldShape, message_name: &str) -> TokenStream {
        match shape {
            FieldShape::Optional => quote! {
                if let Some(existing) = self.os_based_return_url.as_mut() {
                    if !existing.os_type.is_empty() {
                        existing.return_url_map = value.return_url_map;
                    }
                }
            },
            FieldShape::Required => quote! {
                if !self.os_based_return_url.os_type.is_empty() {
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
            }
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
- `SomeMessage field = 1;` becomes `SomeMessage`
- `repeated SomeMessage field = 1;` becomes `Vec<SomeMessage>`

So the setter cannot be one generic assignment for every shape.

The current contract supports these `FieldShape` variants in `setter_body`:

- `FieldShape::Optional`
- `FieldShape::Required`

`repeated` is intentionally rejected with a generated `compile_error!`. This is safer than silently picking behavior for a shape that has not been designed.

## Current `os_based_return_url` Semantics

`os_based_return_url` is optional on the request context, but if it is present, `os_type` must already be present in practice.

The generated setter follows that rule:

- It does not create `os_based_return_url` if the request did not send it.
- It preserves the request-provided `os_type`.
- It only fills `return_url_map` when `os_type` is non-empty.

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

Runtime connector-specific code lives in `grpc-server/src/sanity_layer.rs`.

That runtime layer is trait-shaped per connector. Each connector sanity implementation owns:

- how to extract its raw connector config
- how to apply its sanity behavior to a typed request

For Plaid, that runtime layer:

- resolves a connector name from metadata or raw config
- matches the connector against registered sanity handlers
- reads Euler-only keys from raw `x-connector-config`
- builds an `OsBasedReturnUrl` value containing `return_url_map`
- calls `populate_os_based_return_url`

This keeps generated infrastructure generic and connector-specific policy outside `grpc-api-types`.

Unsupported connectors explicitly go through the default match arm and log that no connector sanity is registered. That makes the runtime structure a connector sanity registry instead of a Plaid-specific predicate.

## Adding A New Auto-Populated Field

To add another field to the generated sanity infrastructure:

1. Add the destination field to the relevant `.proto` message or messages.
2. Add a new variant to `AutoPopulateField`.
3. Add that variant to `AUTO_POPULATE_FIELDS`.
4. Create a small spec type, for example `NativeAppIdentifierSpec`.
5. Implement `AutoPopulateSpec` for that spec type:
   - `field_name`: exact proto field name
   - `trait_name`: generated Rust trait name
   - `method_name`: generated Rust method name
   - `value_type`: Rust type accepted by the generated setter
   - `setter_body`: shape-based setter logic using `quote!`
6. Add delegation for the new variant in `impl AutoPopulateSpec for AutoPopulateField`.
7. Add or update focused tests in `grpc-api-types/tests/auto_populate_test.rs`.
8. Build or test `grpc-api-types`; the generated file in `OUT_DIR` will include
   trait impls for every descriptor-discovered matching message.

Example shape:

```rust
#[derive(Clone, Copy)]
struct NativeAppIdentifierSpec;

impl AutoPopulateSpec for NativeAppIdentifierSpec {
    fn field_name(self) -> &'static str {
        "native_app_identifier"
    }

    fn trait_name(self) -> &'static str {
        "PopulateNativeAppIdentifier"
    }

    fn method_name(self) -> &'static str {
        "populate_native_app_identifier"
    }

    fn value_type(self) -> TokenStream {
        quote! { String }
    }

    fn setter_body(self, shape: FieldShape, message_name: &str) -> TokenStream {
        match shape {
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
            }
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
bounds when it calls those generated methods.

## Adding A New Connector Sanity Handler

Connector-specific sanity logic lives in
`crates/grpc-server/grpc-server/src/sanity_layer.rs`, not in generated code.

To add a new connector:

1. Make sure the connector can be resolved to a `ConnectorVariant` by the
   existing metadata/config parsing path.
2. Add a connector-specific sanity struct, for example `FooPaySanity`.
3. Implement `ConnectorSanity` for that struct:
   - `raw_config` should extract only that connector's raw config from
     `x-connector-config`.
   - `apply` should convert connector-specific config keys into the generic
     request value and call the generated populate method.
4. Add one match arm in `sanity_connector` for the connector enum variant.
5. Add one match arm in `ConnectorSanitizer::sanitize` to call the connector's
   sanity implementation.
6. Leave all unsupported connectors on the `Other(connector)` path; they should
   log `no connector sanity registered` and perform no mutation.
7. Add a focused test or smoke command that sends raw `x-connector-config` for
   the connector and confirms the typed request is populated before the handler
   runs.

Example shape:

```rust
struct FooPaySanity;

impl ConnectorSanity for FooPaySanity {
    fn raw_config(&self, metadata: &MetadataMap) -> Option<serde_json::Value> {
        raw_connector_config(metadata, &["FooPay", "foo_pay"])
    }

    fn apply<T: PopulateOsBasedReturnUrl>(&self, metadata: &MetadataMap, req: &mut T) {
        let Some(config) = self.raw_config(metadata) else {
            return;
        };
        let Some(value) = build_value_from_foo_pay_config(config) else {
            return;
        };

        req.populate_os_based_return_url(value);
    }
}
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
