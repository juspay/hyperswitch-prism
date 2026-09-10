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

## Developer-Written Contract: `FIELD_SPECS`

`FIELD_SPECS` is the only hand-written field declaration surface.

For each field, the developer writes:

- protobuf field name
- generated trait name
- generated method name
- Rust value type
- setter body for optional fields
- setter body for non-optional singular fields

Example:

```rust
FieldSpec {
    field_name: "os_based_return_url",
    trait_name: "PopulateOsBasedReturnUrl",
    method_name: "populate_os_based_return_url",
    value_type: "crate::payments::OsBasedReturnUrl",
    when_optional_body: "...",
    when_required_body: "...",
}
```

The developer does not list request/message types. The descriptor decides where the field exists.

## Why Setter Bodies Are Shape-Based

Different proto field shapes generate different Rust field types:

- `optional SomeMessage field = 1;` becomes `Option<SomeMessage>`
- `SomeMessage field = 1;` becomes `SomeMessage`
- `repeated SomeMessage field = 1;` becomes `Vec<SomeMessage>`

So the setter cannot be one generic assignment for every shape.

The current contract supports:

- `when_optional_body`
- `when_required_body`

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
    async fn some_method(&self, request: tonic::Request<RequestType>) -> Result<...> {
        let (mut metadata, extensions, mut message) = request.into_parts();
        self.sanitizer.sanitize(&mut metadata, &mut message);
        let request = tonic::Request::from_parts(metadata, extensions, message);
        self.inner.some_method(request).await
    }
}
```

Reasoning:

- sanity runs before the real handler
- no flow handler file needs to be modified
- HTTP handlers that call the same service object also pass through the wrapper
- metadata can be cleaned before downstream typed header parsing
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
- Plaid redirect keys can be moved from raw metadata into the typed request

When `connector-sanity-layer` is disabled:

- `crate::sanity_layer::wrap(service)` returns the original service
- there is no generated wrapper in the runtime call path
- normal UCS behavior is unchanged

This keeps integration code stable while avoiding runtime overhead for builds that do not need the compatibility layer.

## Runtime Responsibility

The generated code does not know Plaid, Euler, or merchant-specific config structure.

Runtime connector-specific code lives in `grpc-server/src/sanity_layer.rs`.

For Plaid, that runtime layer:

- identifies Plaid from metadata/raw config
- reads Euler-only keys from raw `x-connector-config`
- builds an `OsBasedReturnUrl` value containing `return_url_map`
- calls `populate_os_based_return_url`
- removes Euler-only keys from metadata before normal typed config parsing

This keeps generated infrastructure generic and connector-specific policy outside `grpc-api-types`.

## Adding A New Auto-Populated Field

To add another field, add one `FieldSpec`.

Required information:

- `field_name`: exact proto field name
- `trait_name`: generated Rust trait name
- `method_name`: generated Rust method name
- `value_type`: Rust type passed to the setter
- `when_optional_body`: assignment logic for `optional` proto fields
- `when_required_body`: assignment logic for non-optional singular proto fields

No request/message list is needed.

After adding the spec:

1. Add the field to the relevant `.proto` messages.
2. Run the build.
3. The generated file in `OUT_DIR` will include trait impls for every matching message.
4. Runtime sanity code can call the generated method generically.

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
