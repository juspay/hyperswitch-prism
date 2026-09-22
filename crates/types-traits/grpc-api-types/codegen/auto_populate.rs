//! Descriptor-driven generator for "populate this field if the request has it"
//! traits.
//!
//! Discovery of *which* request messages contain a given field is entirely
//! derived from the compiled `FileDescriptorSet` (via each service's RPC
//! input types) — never hand-maintained. The only thing a developer writes
//! per field is one enum variant below: field name, generated trait API, value
//! type, and typed setter tokens per field *shape* (optional vs. plain
//! singular) — not a list of message types. Adding a new flow whose request
//! contains the field therefore requires zero changes here; the next build
//! picks it up automatically.

use std::collections::{BTreeMap, BTreeSet};

use heck::{ToSnakeCase, ToUpperCamelCase};
use proc_macro2::{Ident, TokenStream};
use prost_types::{
    field_descriptor_proto::{Label, Type},
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet, ServiceDescriptorProto,
};
use quote::{format_ident, quote};

/// Only the `types` package is aliased as `crate::payments` (see `src/lib.rs`);
/// everything else (e.g. `grpc.health.v1`) is intentionally out of scope for
/// this generator.
const SUPPORTED_PACKAGE: &str = "types";
const SUPPORTED_PACKAGE_MODULE: &str = "crate::payments";

#[derive(Clone, Copy)]
enum FieldShape {
    Optional,
    Required,
    Repeated,
}

/// Registry of auto-populated fields. This is intentionally the only list a
/// developer extends when adding a new generated populate API.
#[derive(Clone, Copy)]
enum AutoPopulateField {
    OsBasedReturnUrl,
}

const AUTO_POPULATE_FIELDS: &[AutoPopulateField] = &[AutoPopulateField::OsBasedReturnUrl];

impl AutoPopulateField {
    fn trait_ident(self) -> Ident {
        format_ident!("{}", self.trait_name())
    }

    fn method_ident(self) -> Ident {
        format_ident!("{}", self.method_name())
    }

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
                    let method = self.method_name();
                    let field_name = self.field_name();
                    let error = format!(
                        "{method}: `{message_name}.{field_name}` exists but is `repeated`, which this generator does not support — add explicit handling in codegen/auto_populate.rs",
                    );
                    quote! {
                        let _ = value;
                        compile_error!(#error);
                    }
                }
            },
        }
    }
}

/// Every distinct request message name used as an RPC input type across every
/// service in the descriptor, restricted to `SUPPORTED_PACKAGE`. This is the
/// authoritative "what counts as a request" list — derived, never maintained
/// by hand.
fn request_message_names(descriptor_set: &FileDescriptorSet) -> BTreeSet<String> {
    descriptor_set
        .file
        .iter()
        .flat_map(|file| file.service.iter())
        .flat_map(|service| service.method.iter())
        .filter_map(|method| method.input_type.as_deref())
        .filter_map(|input_type| {
            let trimmed = input_type.trim_start_matches('.');
            let (package, message_name) = trimmed.rsplit_once('.')?;
            (package == SUPPORTED_PACKAGE).then(|| message_name.to_string())
        })
        .collect()
}

/// Top-level message name -> descriptor, restricted to `SUPPORTED_PACKAGE`.
fn message_index(descriptor_set: &FileDescriptorSet) -> BTreeMap<String, &DescriptorProto> {
    descriptor_set
        .file
        .iter()
        .filter(|file| file.package.as_deref() == Some(SUPPORTED_PACKAGE))
        .flat_map(|file| file.message_type.iter())
        .filter_map(|message| message.name.clone().map(|name| (name, message)))
        .collect()
}

fn message_names(descriptor_set: &FileDescriptorSet) -> BTreeSet<String> {
    message_index(descriptor_set).into_keys().collect()
}

fn rust_type_name(proto_name: &str) -> String {
    proto_name.to_upper_camel_case()
}

fn rust_type_ident(proto_name: &str) -> Ident {
    format_ident!("{}", rust_type_name(proto_name))
}

fn path_tokens(path: &str) -> TokenStream {
    path.parse()
        .unwrap_or_else(|error| panic!("invalid generated path `{path}`: {error}"))
}

fn find_field<'a>(
    message: &'a DescriptorProto,
    field_name: &str,
) -> Option<&'a FieldDescriptorProto> {
    message
        .field
        .iter()
        .find(|field| field.name.as_deref() == Some(field_name))
}

/// prost always generates `Option<T>` for a singular message-typed field,
/// regardless of whether the `.proto` field carries the `optional` keyword —
/// message fields track presence independently of any `optional` marker,
/// unlike scalars. So a message-typed field is always `Optional` shape here;
/// only a scalar's shape actually depends on `proto3_optional`.
fn field_shape(field: &FieldDescriptorProto) -> FieldShape {
    if field.label == Some(Label::Repeated as i32) {
        FieldShape::Repeated
    } else if field.proto3_optional == Some(true) || field.r#type == Some(Type::Message as i32) {
        FieldShape::Optional
    } else {
        FieldShape::Required
    }
}

/// For a message that has the field, decide which spec-supplied setter body
/// applies, based on the field's shape as reported by the descriptor — never
/// guessed from the message name or type.
fn setter_body_for(
    spec: AutoPopulateField,
    field: &FieldDescriptorProto,
    message_name: &str,
) -> TokenStream {
    spec.setter_body(field_shape(field), message_name)
}

fn type_name(path: &str) -> Option<&str> {
    path.trim_start_matches('.')
        .rsplit_once('.')
        .map(|(_, name)| name)
}

fn message_field_type_name(field: &FieldDescriptorProto) -> Option<&str> {
    (field.r#type == Some(Type::Message as i32))
        .then_some(field.type_name.as_deref())
        .flatten()
        .and_then(type_name)
}

fn message_contains_field(
    messages: &BTreeMap<String, &DescriptorProto>,
    message_name: &str,
    field_name: &str,
    seen: &mut BTreeSet<String>,
) -> bool {
    if !seen.insert(message_name.to_string()) {
        return false;
    }

    let Some(message) = messages.get(message_name) else {
        return false;
    };

    find_field(message, field_name).is_some()
        || message.field.iter().any(|field| {
            message_field_type_name(field).is_some_and(|child_name| {
                message_contains_field(messages, child_name, field_name, seen)
            })
        })
}

/// Memoizes `message_contains_field`'s answer per `(message, field)` pair.
///
/// Every call site here (`populate_impl_message_names`, and every
/// `nested_delegate_body` invocation's field scan and oneof-arm re-scan)
/// starts a *fresh*, independent, cycle-safe traversal — each one already
/// computes the objectively correct final answer for `message_name` on its
/// own, via `message_contains_field`'s own `seen` set. This cache only
/// avoids re-running that full traversal when the exact same question gets
/// asked again from a different call site, which happens routinely: the same
/// message gets queried once per registered field in
/// `populate_impl_message_names`, then again per field of every message that
/// needs delegation, then a third time during oneof-arm filtering. It never
/// shares state *within* an in-progress traversal, so it can't change the
/// result of any individual `message_contains_field` call — only how many
/// times that call has to actually run.
type ReachabilityCache = std::cell::RefCell<BTreeMap<(String, &'static str), bool>>;

fn message_reaches_field(
    messages: &BTreeMap<String, &DescriptorProto>,
    message_name: &str,
    field_name: &'static str,
    cache: &ReachabilityCache,
) -> bool {
    let key = (message_name.to_string(), field_name);
    if let Some(&cached) = cache.borrow().get(&key) {
        return cached;
    }
    let result = message_contains_field(messages, message_name, field_name, &mut BTreeSet::new());
    cache.borrow_mut().insert(key, result);
    result
}

fn child_contains_field(
    messages: &BTreeMap<String, &DescriptorProto>,
    field: &FieldDescriptorProto,
    field_name: &'static str,
    cache: &ReachabilityCache,
) -> bool {
    message_field_type_name(field)
        .is_some_and(|child_name| message_reaches_field(messages, child_name, field_name, cache))
}

fn oneof_name(message: &DescriptorProto, field: &FieldDescriptorProto) -> Option<String> {
    let index = usize::try_from(field.oneof_index?).ok()?;
    message
        .oneof_decl
        .get(index)?
        .name
        .as_deref()
        .map(ToSnakeCase::to_snake_case)
}

fn nested_delegate_body(
    spec: AutoPopulateField,
    messages: &BTreeMap<String, &DescriptorProto>,
    message_name: &str,
    message: &DescriptorProto,
    cache: &ReachabilityCache,
) -> Option<TokenStream> {
    let field = message
        .field
        .iter()
        .find(|field| child_contains_field(messages, field, spec.field_name(), cache))?;
    let child_name = message_field_type_name(field)?;

    if field.oneof_index.is_some() {
        let oneof_index = field.oneof_index?;
        let oneof_name = oneof_name(message, field)?;
        let oneof_field_ident = format_ident!("{}", oneof_name);
        let oneof_module_ident = format_ident!("{}", rust_type_name(message_name).to_snake_case());
        let oneof_enum_ident = format_ident!(
            "{}",
            message
                .oneof_decl
                .get(usize::try_from(oneof_index).ok()?)?
                .name
                .as_deref()?
                .to_upper_camel_case()
        );
        let method_ident = spec.method_ident();
        let module = path_tokens(SUPPORTED_PACKAGE_MODULE);
        let arms = message
            .field
            .iter()
            .filter(|field| field.oneof_index == Some(oneof_index))
            .filter(|field| child_contains_field(messages, field, spec.field_name(), cache))
            .filter_map(|field| {
                field.name.as_deref().map(|name| {
                    let variant_ident = format_ident!("{}", name.to_upper_camel_case());
                    quote! {
                        Some(#module::#oneof_module_ident::#oneof_enum_ident::#variant_ident(inner)) => {
                            inner.#method_ident(value);
                        }
                    }
                })
            })
            .collect::<Vec<_>>();

        if arms.is_empty() {
            return None;
        }

        Some(quote! {
            match self.#oneof_field_ident.as_mut() {
                #(#arms,)*
                _ => {
                    let _ = value;
                }
            }
        })
    } else if field.label == Some(Label::Repeated as i32) {
        let error = format!(
            "{method}: `{message}.{field}` can reach `{target}` through repeated message `{child}`, which this generator does not support yet",
            method = spec.method_name(),
            message = message_name,
            field = field.name.as_deref().unwrap_or("<unknown>"),
            target = spec.field_name(),
            child = child_name,
        );
        Some(quote! {
            compile_error!(#error);
            let _ = value;
        })
    } else {
        let field_ident = format_ident!("{}", field.name.as_deref()?);
        let method_ident = spec.method_ident();
        Some(quote! {
            if let Some(inner) = self.#field_ident.as_mut() {
                inner.#method_ident(value);
            } else {
                let _ = value;
            }
        })
    }
}

fn populate_impl_message_names(
    descriptor_set: &FileDescriptorSet,
    cache: &ReachabilityCache,
) -> BTreeSet<String> {
    let messages = message_index(descriptor_set);
    let mut names = request_message_names(descriptor_set);

    names.extend(
        message_names(descriptor_set)
            .into_iter()
            .filter(|message_name| {
                AUTO_POPULATE_FIELDS.iter().any(|spec| {
                    message_reaches_field(&messages, message_name, spec.field_name(), cache)
                })
            }),
    );

    names
}

fn supported_services(descriptor_set: &FileDescriptorSet) -> Vec<&ServiceDescriptorProto> {
    descriptor_set
        .file
        .iter()
        .filter(|file| file.package.as_deref() == Some(SUPPORTED_PACKAGE))
        .flat_map(|file| file.service.iter())
        .filter(|service| service.name.is_some())
        .collect()
}

fn generate_sanity_layer(descriptor_set: &FileDescriptorSet) -> TokenStream {
    let sanitizer_bounds = AUTO_POPULATE_FIELDS
        .iter()
        .map(|spec| spec.trait_ident())
        .collect::<Vec<_>>();
    let service_impls = supported_services(descriptor_set)
        .into_iter()
        .map(|service| {
            let service_name = service.name.as_deref().expect("service name was filtered");
            let service_ident = format_ident!("{}", service_name);
            let service_module_ident = format_ident!("{}_server", service_name.to_snake_case());
            let module = path_tokens(SUPPORTED_PACKAGE_MODULE);
            let method_impls = service
                .method
                .iter()
                .filter_map(|method| {
                    let method_ident = format_ident!("{}", method.name.as_deref()?.to_snake_case());
                    let input_ident =
                        rust_type_ident(method.input_type.as_deref().and_then(type_name)?);
                    let output_ident =
                        rust_type_ident(method.output_type.as_deref().and_then(type_name)?);
                    Some(quote! {
                        async fn #method_ident(
                            &self,
                            mut request: tonic::Request<#module::#input_ident>,
                        ) -> Result<tonic::Response<#module::#output_ident>, tonic::Status> {
                            let metadata = request.metadata().clone();
                            self.sanitizer.sanitize(&metadata, request.get_mut());
                            self.inner.#method_ident(request).await
                        }
                    })
                })
                .collect::<Vec<_>>();

            quote! {
                #[tonic::async_trait]
                impl<S, Z> #module::#service_module_ident::#service_ident for SanityLayer<S, Z>
                where
                    S: #module::#service_module_ident::#service_ident,
                    Z: RequestSanitizer,
                {
                    #(#method_impls)*
                }
            }
        })
        .collect::<Vec<_>>();

    quote! {
        #[derive(Clone, Copy, Debug, Default)]
        pub struct NoopSanitizer;

        pub trait RequestSanitizer: Clone + Send + Sync + 'static {
            fn sanitize<T: #(#sanitizer_bounds)+*>(
                &self,
                metadata: &tonic::metadata::MetadataMap,
                request: &mut T,
            );
        }

        impl RequestSanitizer for NoopSanitizer {
            fn sanitize<T: #(#sanitizer_bounds)+*>(
                &self,
                _metadata: &tonic::metadata::MetadataMap,
                _request: &mut T,
            ) {
            }
        }

        #[derive(Clone)]
        pub struct SanityLayer<S, Z = NoopSanitizer> {
            pub inner: S,
            pub sanitizer: Z,
        }

        impl<S, Z> SanityLayer<S, Z> {
            pub fn new(inner: S, sanitizer: Z) -> Self {
                Self { inner, sanitizer }
            }
        }

        impl<S> SanityLayer<S, NoopSanitizer> {
            pub fn noop(inner: S) -> Self {
                Self { inner, sanitizer: NoopSanitizer }
            }
        }

        #(#service_impls)*
    }
}

pub fn generate(descriptor_set: &FileDescriptorSet) -> String {
    let messages = message_index(descriptor_set);
    let cache = ReachabilityCache::default();
    let message_names = populate_impl_message_names(descriptor_set, &cache);
    let module = path_tokens(SUPPORTED_PACKAGE_MODULE);

    let trait_impls = AUTO_POPULATE_FIELDS
        .iter()
        .flat_map(|spec| {
            let trait_ident = spec.trait_ident();
            let method_ident = spec.method_ident();
            let value_type = spec.value_type();
            // Default body covers every message with no real path to the
            // field — those messages get a bare `impl Trait for Message {}`
            // below, relying on this default, instead of each one repeating
            // `let _ = value;` in its own generated method.
            let trait_def = quote! {
                pub trait #trait_ident {
                    fn #method_ident(&mut self, value: #value_type) {
                        let _ = value;
                    }
                }
            };

            let impls = message_names
                .iter()
                .filter_map(|message_name| {
                    let descriptor = messages.get(message_name)?;
                    let body = match find_field(descriptor, spec.field_name()) {
                        Some(field) => Some(setter_body_for(*spec, field, message_name)),
                        None => {
                            nested_delegate_body(*spec, &messages, message_name, descriptor, &cache)
                        }
                    };
                    let message_ident = rust_type_ident(message_name);

                    Some(match body {
                        Some(body) => quote! {
                            impl #trait_ident for #module::#message_ident {
                                fn #method_ident(&mut self, value: #value_type) {
                                    #body
                                }
                            }
                        },
                        None => quote! {
                            impl #trait_ident for #module::#message_ident {}
                        },
                    })
                })
                .collect::<Vec<_>>();

            std::iter::once(trait_def).chain(impls)
        })
        .collect::<Vec<_>>();
    let sanity_layer = generate_sanity_layer(descriptor_set);

    let tokens = quote! {
        pub mod generated {
            #![allow(clippy::all)]

            #(#trait_impls)*

            #sanity_layer
        }
    };
    let generated = tokens.to_string();
    let generated = syn::parse_file(&generated)
        .map(|file| prettyplease::unparse(&file))
        .unwrap_or(generated);

    format!(
        "// GENERATED FILE - DO NOT EDIT MANUALLY\n// Generated by codegen/auto_populate.rs from the compiled protobuf descriptor set.\n{}",
        generated
    )
}
