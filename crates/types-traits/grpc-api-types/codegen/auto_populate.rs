//! Descriptor-driven generator for "populate this field if the request has it"
//! traits.
//!
//! Discovery of *which* request messages contain a given field is entirely
//! derived from the compiled `FileDescriptorSet` (via each service's RPC
//! input types) — never hand-maintained. The only thing a developer writes
//! per field is, in `FIELD_SPECS` below, the setter body for each possible
//! field *shape* (optional vs. plain singular) — not a list of message
//! types. Adding a new flow whose request contains the field therefore
//! requires zero changes here; the next build picks it up automatically.

use std::collections::{BTreeMap, BTreeSet};

use heck::{ToSnakeCase, ToUpperCamelCase};
use prost_types::{
    field_descriptor_proto::{Label, Type},
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet, ServiceDescriptorProto,
};

/// Only the `types` package is aliased as `crate::payments` (see `src/lib.rs`);
/// everything else (e.g. `grpc.health.v1`) is intentionally out of scope for
/// this generator.
const SUPPORTED_PACKAGE: &str = "types";
const SUPPORTED_PACKAGE_MODULE: &str = "crate::payments";

/// The entire developer-facing declaration surface: a field name, and one
/// setter body per field shape. Nothing about *which messages* have the
/// field is declared here.
pub struct FieldSpec {
    pub field_name: &'static str,
    pub trait_name: &'static str,
    pub method_name: &'static str,
    pub value_type: &'static str,
    /// Setter body for a field declared with the `optional` keyword
    /// (prost emits `Option<value_type>`). May reference `req` and `value`.
    pub when_optional_body: &'static str,
    /// Setter body for a plain singular field (prost emits `value_type`
    /// directly, no `Option`). May reference `req` and `value`.
    pub when_required_body: &'static str,
}

pub const FIELD_SPECS: &[FieldSpec] = &[
    FieldSpec {
        field_name: "os_based_return_url",
        trait_name: "PopulateOsBasedReturnUrl",
        method_name: "populate_os_based_return_url",
        value_type: "crate::payments::OsBasedReturnUrl",
        when_optional_body: "if let Some(existing) = req.os_based_return_url.as_mut() {\n                if !existing.os_type.is_empty() {\n                    existing.return_url_map = value.return_url_map;\n                }\n            }",
        when_required_body: "if !req.os_based_return_url.os_type.is_empty() {\n                req.os_based_return_url.return_url_map = value.return_url_map;\n            }",
    },
];

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

fn find_field<'a>(
    message: &'a DescriptorProto,
    field_name: &str,
) -> Option<&'a FieldDescriptorProto> {
    message
        .field
        .iter()
        .find(|field| field.name.as_deref() == Some(field_name))
}

/// For a message that has the field, decide which developer-supplied setter
/// body applies, based on the field's shape as reported by the descriptor —
/// never guessed from the message name or type.
fn setter_body_for(spec: &FieldSpec, field: &FieldDescriptorProto, message_name: &str) -> String {
    if field.label == Some(Label::Repeated as i32) {
        return format!(
            "let _ = value; compile_error!(\"{method}: `{message}.{field_name}` exists but is `repeated`, which this generator does not support — add explicit handling in build/auto_populate.rs\");",
            method = spec.method_name,
            message = message_name,
            field_name = spec.field_name,
        );
    }

    let body = if field.proto3_optional == Some(true) {
        spec.when_optional_body
    } else {
        spec.when_required_body
    };
    format!("let req = self;\n            {body}")
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

fn child_contains_field(
    messages: &BTreeMap<String, &DescriptorProto>,
    field: &FieldDescriptorProto,
    field_name: &str,
) -> bool {
    message_field_type_name(field).is_some_and(|child_name| {
        message_contains_field(messages, child_name, field_name, &mut BTreeSet::new())
    })
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
    spec: &FieldSpec,
    messages: &BTreeMap<String, &DescriptorProto>,
    message_name: &str,
    message: &DescriptorProto,
) -> Option<String> {
    let field = message
        .field
        .iter()
        .find(|field| child_contains_field(messages, field, spec.field_name))?;
    let child_name = message_field_type_name(field)?;

    if field.oneof_index.is_some() {
        let oneof_index = field.oneof_index?;
        let oneof_name = oneof_name(message, field)?;
        let oneof_module = rust_type_name(message_name).to_snake_case();
        let oneof_enum_name = message
            .oneof_decl
            .get(usize::try_from(oneof_index).ok()?)?
            .name
            .as_deref()?
            .to_upper_camel_case();
        let arms = message
            .field
            .iter()
            .filter(|field| field.oneof_index == Some(oneof_index))
            .filter(|field| child_contains_field(messages, field, spec.field_name))
            .filter_map(|field| {
                field.name.as_deref().map(|name| {
                    format!(
                        "Some({module}::{oneof_module}::{oneof_enum_name}::{variant}(inner)) => {{\n                    inner.{method}(value);\n                }}",
                        module = SUPPORTED_PACKAGE_MODULE,
                        oneof_module = oneof_module,
                        oneof_enum_name = oneof_enum_name,
                        variant = name.to_upper_camel_case(),
                        method = spec.method_name,
                    )
                })
            })
            .collect::<Vec<_>>()
            .join(",\n                ");

        if arms.is_empty() {
            return None;
        }

        Some(format!(
            "match self.{oneof_name}.as_mut() {{\n                {arms},\n                _ => {{\n                    let _ = value;\n                }}\n            }}",
            oneof_name = oneof_name,
            arms = arms,
        ))
    } else if field.label == Some(Label::Repeated as i32) {
        Some(format!(
            "compile_error!(\"{method}: `{message}.{field}` can reach `{target}` through repeated message `{child}`, which this generator does not support yet\");\n            let _ = value;",
            method = spec.method_name,
            message = message_name,
            field = field.name.as_deref().unwrap_or("<unknown>"),
            target = spec.field_name,
            child = child_name,
        ))
    } else {
        let field_name = field.name.as_deref()?;
        Some(format!(
            "if let Some(inner) = self.{field_name}.as_mut() {{\n                inner.{method}(value);\n            }} else {{\n                let _ = value;\n            }}",
            field_name = field_name,
            method = spec.method_name,
        ))
    }
}

fn populate_impl_message_names(descriptor_set: &FileDescriptorSet) -> BTreeSet<String> {
    let messages = message_index(descriptor_set);
    let mut names = request_message_names(descriptor_set);

    names.extend(
        message_names(descriptor_set)
            .into_iter()
            .filter(|message_name| {
                FIELD_SPECS.iter().any(|spec| {
                    message_contains_field(
                        &messages,
                        message_name,
                        spec.field_name,
                        &mut BTreeSet::new(),
                    )
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

fn generate_sanity_layer(out: &mut String, descriptor_set: &FileDescriptorSet) {
    let sanitizer_bounds = FIELD_SPECS
        .iter()
        .map(|spec| spec.trait_name)
        .collect::<Vec<_>>()
        .join(" + ");

    out.push_str("    #[derive(Clone, Copy, Debug, Default)]\n    pub struct NoopSanitizer;\n\n");
    out.push_str(&format!(
        "    pub trait RequestSanitizer: Clone + Send + Sync + 'static {{\n        fn sanitize<T: {sanitizer_bounds}>(&self, metadata: &mut tonic::metadata::MetadataMap, request: &mut T);\n    }}\n\n",
    ));
    out.push_str(&format!(
        "    impl RequestSanitizer for NoopSanitizer {{\n        fn sanitize<T: {sanitizer_bounds}>(&self, _metadata: &mut tonic::metadata::MetadataMap, _request: &mut T) {{}}\n    }}\n\n",
    ));
    out.push_str(
        "    #[derive(Clone)]\n    pub struct SanityLayer<S, Z = NoopSanitizer> {\n        pub inner: S,\n        pub sanitizer: Z,\n    }\n\n",
    );
    out.push_str(
        "    impl<S, Z> SanityLayer<S, Z> {\n        pub fn new(inner: S, sanitizer: Z) -> Self {\n            Self { inner, sanitizer }\n        }\n    }\n\n",
    );
    out.push_str(
        "    impl<S> SanityLayer<S, NoopSanitizer> {\n        pub fn noop(inner: S) -> Self {\n            Self { inner, sanitizer: NoopSanitizer }\n        }\n    }\n\n",
    );

    for service in supported_services(descriptor_set) {
        let service_name = service.name.as_deref().expect("service name was filtered");
        let service_module = format!("{}_server", service_name.to_snake_case());

        out.push_str(&format!(
            "    #[tonic::async_trait]\n    impl<S, Z> {module}::{service} for SanityLayer<S, Z>\n    where\n        S: {module}::{service},\n        Z: RequestSanitizer,\n    {{\n",
            module = format!("{SUPPORTED_PACKAGE_MODULE}::{service_module}"),
            service = service_name,
        ));

        for method in &service.method {
            let Some(method_name) = method.name.as_deref() else {
                continue;
            };
            let Some(input_type) = method.input_type.as_deref().and_then(type_name) else {
                continue;
            };
            let Some(output_type) = method.output_type.as_deref().and_then(type_name) else {
                continue;
            };
            let rust_method_name = method_name.to_snake_case();

            out.push_str(&format!(
                "        async fn {method}(\n            &self,\n            request: tonic::Request<{types_module}::{input_type}>,\n        ) -> Result<tonic::Response<{types_module}::{output_type}>, tonic::Status> {{\n            let (mut metadata, extensions, mut message) = request.into_parts();\n            self.sanitizer.sanitize(&mut metadata, &mut message);\n            let request = tonic::Request::from_parts(metadata, extensions, message);\n            self.inner.{method}(request).await\n        }}\n\n",
                method = rust_method_name,
                types_module = SUPPORTED_PACKAGE_MODULE,
                input_type = rust_type_name(input_type),
                output_type = rust_type_name(output_type),
            ));
        }

        out.push_str("    }\n\n");
    }
}

pub fn generate(descriptor_set: &FileDescriptorSet) -> String {
    let messages = message_index(descriptor_set);
    let message_names = populate_impl_message_names(descriptor_set);

    let mut out = String::new();
    out.push_str("// GENERATED FILE - DO NOT EDIT MANUALLY\n");
    out.push_str(
        "// Generated by codegen/auto_populate.rs from the compiled protobuf descriptor set.\n",
    );
    out.push_str("pub mod generated {\n");
    out.push_str("    #![allow(clippy::all)]\n\n");

    for spec in FIELD_SPECS {
        out.push_str(&format!(
            "    pub trait {trait_name} {{\n        fn {method}(&mut self, value: {value_type});\n    }}\n\n",
            trait_name = spec.trait_name,
            method = spec.method_name,
            value_type = spec.value_type,
        ));

        for message_name in &message_names {
            let Some(descriptor) = messages.get(message_name) else {
                continue;
            };

            let body = match find_field(descriptor, spec.field_name) {
                Some(field) => setter_body_for(spec, field, message_name),
                None => nested_delegate_body(spec, &messages, message_name, descriptor)
                    .unwrap_or_else(|| "let _ = value;".to_string()),
            };

            out.push_str(&format!(
                "    impl {trait_name} for {module}::{message} {{\n        fn {method}(&mut self, value: {value_type}) {{\n            {body}\n        }}\n    }}\n\n",
                trait_name = spec.trait_name,
                module = SUPPORTED_PACKAGE_MODULE,
                message = rust_type_name(message_name),
                method = spec.method_name,
                value_type = spec.value_type,
                body = body,
            ));
        }
    }

    generate_sanity_layer(&mut out, descriptor_set);

    out.push_str("}\n");
    out
}
