use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote, ToTokens};
use syn::{
    parse_quote, punctuated::Punctuated, Attribute, GenericArgument, Meta, PathArguments, Token,
    Type, TypePath,
};

use crate::generics::GenericPatchCtx;

pub(crate) struct FieldSpec {
    // Field name in the source struct.
    pub(crate) ident: syn::Ident,
    // Doc attributes copied onto the patch field.
    pub(crate) doc_attrs: Vec<Attribute>,
    // Serde attributes copied onto the patch field.
    pub(crate) serde_attrs: Vec<Attribute>,
    // Patch field type to emit.
    pub(crate) patch_field_ty: TokenStream2,
    // Apply-statement tokens for this field.
    pub(crate) apply_stmt: TokenStream2,
}

struct FieldPatchConfig {
    ignore: bool,
    patch_type: Option<Type>,
}

enum FieldPatchKind<'a> {
    Replace,
    Nested,
    OptionalReplace(&'a Type),
    OptionalNested(&'a Type),
    Unsupported { detail: String },
}

// Build the patch field spec and apply statement for a single field.
pub(crate) fn build_patch_field_specific_metadata(
    field: &syn::Field,
    patch_ctx: &mut GenericPatchCtx,
) -> syn::Result<Option<FieldSpec>> {
    let field_ident = field
        .ident
        .clone()
        .ok_or_else(|| syn::Error::new_spanned(field, "expected named field for patch"))?;

    let field_config = get_field_patch_config(&field.attrs)?;

    if field_config.ignore {
        return Ok(None);
    }

    let patch_type_override = field_config.patch_type.as_ref();
    let field_kind = field_patch_kind(&field.ty, patch_type_override);

    let doc_attrs = match field_kind {
        FieldPatchKind::Unsupported { ref detail } => {
            return Err(nested_unsupported_error(&field_ident, &field.ty, detail))
        }
        _ => field_doc_attrs(&field_ident, &field_kind),
    };

    let (patch_field_ty, apply_stmt) = build_patch_field_specification(
        &field_ident,
        &field.ty,
        &field_kind,
        patch_ctx,
        patch_type_override,
    )?;

    let serde_attrs = build_serde_attributes(&field.attrs, &field.ty, &patch_field_ty)?;

    Ok(Some(FieldSpec {
        ident: field_ident,
        doc_attrs,
        serde_attrs,
        patch_field_ty,
        apply_stmt,
    }))
}

// Parse #[patch(...)] attributes for a field.
fn get_field_patch_config(attrs: &[Attribute]) -> syn::Result<FieldPatchConfig> {
    let mut ignore = false;
    let mut patch_type = None;
    let mut patch_attr = None;

    for attr in attrs {
        if !attr.path().is_ident("patch") {
            continue;
        }
        patch_attr = Some(attr);
        if let Meta::List(meta) = &attr.meta {
            if meta.tokens.is_empty() {
                continue;
            }
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("nested") || meta.path.is_ident("replace") {
                return Err(meta.error(
                    "patch(nested) and patch(replace) are no longer supported; patch behavior is inferred from field types (use patch_type or ignore)",
                ));
            }

            if meta.path.is_ident("ignore") {
                ignore = true;
                return Ok(());
            }

            if meta.path.is_ident("patch_type") {
                let value = meta.value()?;
                let ty: Type = value.parse()?;
                if patch_type.is_some() {
                    return Err(meta.error("patch_type cannot be set more than once"));
                }
                patch_type = Some(ty);
                return Ok(());
            }

            Err(meta.error("unsupported patch field attribute"))
        })?;
    }

    if ignore && patch_type.is_some() {
        let span_attr = patch_attr.or_else(|| attrs.first());
        match span_attr {
            Some(attr) => Err(syn::Error::new_spanned(
                attr,
                "patch(ignore) cannot be combined with patch_type",
            )),
            None => Err(syn::Error::new(
                Span::call_site(),
                "patch(ignore) cannot be combined with patch_type",
            )),
        }
    } else {
        Ok(FieldPatchConfig { ignore, patch_type })
    }
}

fn field_patch_kind<'a>(
    field_ty: &'a Type,
    patch_type_override: Option<&'a Type>,
) -> FieldPatchKind<'a> {
    match option_inner_type(field_ty) {
        Some(inner_ty) => {
            if option_inner_type(inner_ty).is_some() {
                FieldPatchKind::Unsupported {
                    detail: format!("`{}` (single layer only)", type_display(inner_ty)),
                }
            } else if patch_type_override.is_some() {
                if is_replaceable_scalar(inner_ty) || is_replaceable_container(inner_ty) {
                    FieldPatchKind::Unsupported {
                        detail: "patch_type can only be used with nested patchable types"
                            .to_string(),
                    }
                } else {
                    FieldPatchKind::OptionalNested(inner_ty)
                }
            } else {
                match classify_non_optional(inner_ty) {
                    FieldPatchKind::Replace => FieldPatchKind::OptionalReplace(inner_ty),
                    FieldPatchKind::Nested => FieldPatchKind::OptionalNested(inner_ty),
                    FieldPatchKind::Unsupported { detail } => {
                        FieldPatchKind::Unsupported { detail }
                    }
                    _ => FieldPatchKind::Unsupported {
                        detail: format!("`{}`", type_display(inner_ty)),
                    },
                }
            }
        }
        None => {
            if patch_type_override.is_some() {
                if is_replaceable_scalar(field_ty) || is_replaceable_container(field_ty) {
                    FieldPatchKind::Unsupported {
                        detail: "patch_type can only be used with nested patchable types"
                            .to_string(),
                    }
                } else {
                    FieldPatchKind::Nested
                }
            } else {
                classify_non_optional(field_ty)
            }
        }
    }
}

fn classify_non_optional(field_ty: &Type) -> FieldPatchKind<'_> {
    match field_ty {
        Type::Path(path) if path.qself.is_some() => FieldPatchKind::Unsupported {
            detail: "qualified self type; use patch_type to specify the patch type".to_string(),
        },
        Type::Path(_) if is_replaceable_scalar(field_ty) => FieldPatchKind::Replace,
        Type::Path(_) if is_replaceable_container(field_ty) => FieldPatchKind::Replace,
        Type::Path(_) if has_type_args(field_ty) => FieldPatchKind::Unsupported {
            detail: format!("`{}`", type_display(field_ty)),
        },
        Type::Path(_) => FieldPatchKind::Nested,
        _ => FieldPatchKind::Unsupported {
            detail: format!("`{}`", type_display(field_ty)),
        },
    }
}

fn is_replaceable_scalar(ty: &Type) -> bool {
    replaceable_scalar_ident(ty).is_some()
}

fn is_replaceable_container(ty: &Type) -> bool {
    replaceable_container_ident(ty).is_some()
}

fn replaceable_container_ident(ty: &Type) -> Option<String> {
    let path = match ty {
        Type::Path(path) if path.qself.is_none() => Some(path),
        _ => None,
    }?;

    let last = path.path.segments.last()?;
    let name = last.ident.to_string();
    let is_allowed = matches!(
        name.as_str(),
        "Vec" | "HashMap" | "BTreeMap" | "HashSet" | "BTreeSet"
    );

    match (is_allowed, &last.arguments) {
        (true, PathArguments::AngleBracketed(args)) if !args.args.is_empty() => Some(name),
        _ => None,
    }
}

// Produce a doc string describing the patch field behavior.
fn field_doc_attrs(field_ident: &syn::Ident, kind: &FieldPatchKind<'_>) -> Vec<Attribute> {
    let detail = match kind {
        FieldPatchKind::OptionalNested(_) => "Optional nested patch field. Missing means no change; null clears; value applies nested patch, inserting Default when needed.",
        FieldPatchKind::Nested => "Nested patch field. Missing or null means no change; value applies nested patch.",
        FieldPatchKind::OptionalReplace(_) => "Optional replace-only patch field. Missing means no change; null clears; value replaces the field.",
        FieldPatchKind::Replace => "Replace-only patch field. Missing or null means no change; value replaces the field.",
        FieldPatchKind::Unsupported { .. } => "Patch field.",
    };

    let message = format!("Patch field for `{}`. {}", field_ident, detail);
    vec![parse_quote!(#[doc = #message])]
}

// Build patch field type and apply logic for nested vs replace fields.
fn build_patch_field_specification(
    field_ident: &syn::Ident,
    field_ty: &Type,
    field_kind: &FieldPatchKind<'_>,
    patch_ctx: &mut GenericPatchCtx,
    patch_type_override: Option<&Type>,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    match field_kind {
        FieldPatchKind::Replace => build_replace_spec(field_ident, field_ty, patch_ctx),
        FieldPatchKind::OptionalReplace(inner_ty) => {
            build_optional_replace_spec(field_ident, field_ty, inner_ty, patch_ctx)
        }
        FieldPatchKind::Nested => build_patch_specification_for_plain_nested_field(
            field_ident,
            field_ty,
            patch_ctx,
            patch_type_override,
        ),
        FieldPatchKind::OptionalNested(inner_ty) => {
            build_patch_specification_for_optional_nested_field(
                field_ident,
                inner_ty,
                patch_ctx,
                patch_type_override,
            )
        }
        FieldPatchKind::Unsupported { detail } => {
            Err(nested_unsupported_error(field_ident, field_ty, detail))
        }
    }
}

fn build_replace_spec(
    field_ident: &syn::Ident,
    field_ty: &Type,
    patch_ctx: &mut GenericPatchCtx,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    patch_ctx.record_used_type_params_for_the_field(field_ty);
    Ok((
        quote! { ::core::option::Option<#field_ty> },
        quote! {
            if let ::core::option::Option::Some(value) = patch.#field_ident {
                self.#field_ident = value;
            }
        },
    ))
}

fn build_optional_replace_spec(
    field_ident: &syn::Ident,
    field_ty: &Type,
    inner_ty: &Type,
    patch_ctx: &mut GenericPatchCtx,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    patch_ctx.record_used_type_params_for_the_field(field_ty);
    Ok((
        quote! { ::core::option::Option<::core::option::Option<#inner_ty>> },
        quote! {
            if let ::core::option::Option::Some(value) = patch.#field_ident {
                self.#field_ident = value;
            }
        },
    ))
}

fn nested_unsupported_error(field_ident: &syn::Ident, field_ty: &Type, detail: &str) -> syn::Error {
    syn::Error::new_spanned(
        field_ty,
        format!("field `{field_ident}`: patch not supported for {detail}; use #[patch(ignore)]"),
    )
}

fn optional_nested_apply_stmt(field_ident: &syn::Ident) -> TokenStream2 {
    quote! {
        match patch.#field_ident {
            ::core::option::Option::None => {}
            ::core::option::Option::Some(::core::option::Option::None) => {
                self.#field_ident = ::core::option::Option::None;
            }
            ::core::option::Option::Some(::core::option::Option::Some(patch_value)) => {
                let mut value = self.#field_ident.take().unwrap_or_default();
                ::common_utils::config_patch::Patch::apply(&mut value, patch_value);
                self.#field_ident = ::core::option::Option::Some(value);
            }
        }
    }
}

fn plain_nested_apply_stmt(field_ident: &syn::Ident) -> TokenStream2 {
    quote! {
        if let ::core::option::Option::Some(value) = patch.#field_ident {
            ::common_utils::config_patch::Patch::apply(&mut self.#field_ident, value);
        }
    }
}

// Build patch spec for Option<T> nested patching fields.
fn build_patch_specification_for_optional_nested_field(
    field_ident: &syn::Ident,
    inner_ty: &Type,
    patch_ctx: &mut GenericPatchCtx,
    patch_type_override: Option<&Type>,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    match patch_type_override {
        Some(patch_type_override) => {
            patch_ctx.record_used_type_params_for_the_field(patch_type_override);
            patch_ctx.add_type_bound(inner_ty, patch_type_override, true);
            Ok((
                quote! { ::core::option::Option<::core::option::Option<#patch_type_override>> },
                optional_nested_apply_stmt(field_ident),
            ))
        }
        None => {
            let generic_ident = generic_param_ident(inner_ty, &patch_ctx.generic_params);
            match generic_ident {
                Some(ident) => {
                    let patch_ident = patch_ctx.patch_ident(&ident);
                    patch_ctx.add_bound(&ident, &patch_ident, true);
                    Ok((
                        quote! { ::core::option::Option<::core::option::Option<#patch_ident>> },
                        optional_nested_apply_stmt(field_ident),
                    ))
                }
                None => match patch_type(inner_ty) {
                    Ok(patch_ty) => {
                        patch_ctx.add_type_bound(inner_ty, &patch_ty, true);
                        Ok((
                            quote! { ::core::option::Option<::core::option::Option<#patch_ty>> },
                            optional_nested_apply_stmt(field_ident),
                        ))
                    }
                    Err(err) => Err(err),
                },
            }
        }
    }
}

// Build patch spec for non-Option nested patching fields.
fn build_patch_specification_for_plain_nested_field(
    field_ident: &syn::Ident,
    field_ty: &Type,
    patch_ctx: &mut GenericPatchCtx,
    patch_type_override: Option<&Type>,
) -> syn::Result<(TokenStream2, TokenStream2)> {
    match patch_type_override {
        Some(patch_type_override) => {
            patch_ctx.record_used_type_params_for_the_field(patch_type_override);
            patch_ctx.add_type_bound(field_ty, patch_type_override, false);
            Ok((
                quote! { ::core::option::Option<#patch_type_override> },
                plain_nested_apply_stmt(field_ident),
            ))
        }
        None => {
            let generic_ident = generic_param_ident(field_ty, &patch_ctx.generic_params);
            match generic_ident {
                Some(ident) => {
                    let patch_ident = patch_ctx.patch_ident(&ident);
                    patch_ctx.add_bound(&ident, &patch_ident, false);
                    Ok((
                        quote! { ::core::option::Option<#patch_ident> },
                        plain_nested_apply_stmt(field_ident),
                    ))
                }
                None => match patch_type(field_ty) {
                    Ok(patch_ty) => {
                        patch_ctx.add_type_bound(field_ty, &patch_ty, false);
                        Ok((
                            quote! { ::core::option::Option<#patch_ty> },
                            plain_nested_apply_stmt(field_ident),
                        ))
                    }
                    Err(err) => Err(err),
                },
            }
        }
    }
}

/// Builds serde attributes including custom deserializers for Option<Option<T>>
fn build_serde_attributes(
    field_attrs: &[Attribute],
    field_ty: &Type,
    patch_field_ty: &proc_macro2::TokenStream,
) -> syn::Result<Vec<Attribute>> {
    let mut serde_attrs = serde_field_attrs(field_attrs);
    let serde_flags = serde_attr_flags(&serde_attrs);

    // Add custom deserializer for Option<T> types if needed.
    if option_inner_type(field_ty).is_some() && !serde_flags.has_deserialize_with {
        add_option_deserializer(&mut serde_attrs);

        // Add bound if needed.
        if !serde_flags.has_bound {
            if let Some(bound_attr) = create_deserialize_bound(patch_field_ty)? {
                serde_attrs.push(bound_attr);
            }
        }
    }

    Ok(serde_attrs)
}

/// Adds the custom Option<Option<T>> deserializer attribute
fn add_option_deserializer(serde_attrs: &mut Vec<Attribute>) {
    serde_attrs.push(parse_quote!(
        #[serde(
            deserialize_with = "::common_utils::config_patch::deserialize_option_option"
        )]
    ));
}

/// Creates a deserialize bound attribute for the inner type
fn create_deserialize_bound(
    patch_field_ty: &proc_macro2::TokenStream,
) -> syn::Result<Option<Attribute>> {
    let ty = syn::parse2::<Type>(patch_field_ty.clone()).ok();
    let bound_ty = ty.and_then(|ty| option_option_inner_type(&ty).cloned());

    if let Some(bound_ty) = bound_ty {
        let bound_tokens = quote!(#bound_ty: ::serde::Deserialize<'de>);
        let bound_lit = syn::LitStr::new(&bound_tokens.to_string(), Span::call_site());
        Ok(Some(parse_quote!(
            #[serde(bound(deserialize = #bound_lit))]
        )))
    } else {
        Ok(None)
    }
}

// Copy serde attributes, excluding serde(default).
fn serde_field_attrs(attrs: &[Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
        .filter_map(strip_serde_default)
        .collect()
}

// Check if a field already specifies a serde deserializer.
struct SerdeAttrFlags {
    has_deserialize_with: bool,
    has_bound: bool,
}

fn serde_attr_flags(serde_attrs: &[Attribute]) -> SerdeAttrFlags {
    let mut flags = SerdeAttrFlags {
        has_deserialize_with: false,
        has_bound: false,
    };

    for attr in serde_attrs {
        let parsed = attr
            .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
            .ok();
        let Some(args) = parsed else {
            continue;
        };

        for meta in args {
            match meta {
                Meta::NameValue(nv)
                    if nv.path.is_ident("deserialize_with") || nv.path.is_ident("with") =>
                {
                    flags.has_deserialize_with = true;
                }
                Meta::List(list) if list.path.is_ident("bound") => {
                    flags.has_bound = true;
                }
                Meta::NameValue(nv) if nv.path.is_ident("bound") => {
                    flags.has_bound = true;
                }
                _ => {}
            }
        }
    }

    flags
}

// Remove serde(default) from an attribute list if present.
fn strip_serde_default(attr: &Attribute) -> Option<Attribute> {
    let parsed = attr
        .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        .ok();

    match parsed {
        None => Some(attr.clone()),
        Some(args) => {
            let mut filtered = Punctuated::<Meta, Token![,]>::new();
            for meta in args {
                match &meta {
                    Meta::Path(path) if path.is_ident("default") => {}
                    Meta::NameValue(nv) if nv.path.is_ident("default") => {}
                    _ => filtered.push(meta),
                }
            }

            match filtered.is_empty() {
                true => None,
                false => Some(syn::parse_quote!(#[serde(#filtered)])),
            }
        }
    }
}

// Return the inner type if this is Option<T>.
fn option_inner_type(ty: &Type) -> Option<&Type> {
    let segment = match ty {
        Type::Path(path) => Some(path),
        _ => None,
    }
    .filter(|path| path.qself.is_none())
    .and_then(|path| path.path.segments.last());

    let args = segment.and_then(|segment| match segment.ident == "Option" {
        true => match &segment.arguments {
            PathArguments::AngleBracketed(args) if args.args.len() == 1 => Some(args),
            _ => None,
        },
        false => None,
    });

    args.and_then(|args| match args.args.first() {
        Some(GenericArgument::Type(inner)) => Some(inner),
        _ => None,
    })
}

// Return the inner type if this is Option<Option<T>>.
fn option_option_inner_type(ty: &Type) -> Option<&Type> {
    option_inner_type(ty).and_then(option_inner_type)
}

// Detect if a type path segment carries generic arguments.
fn has_type_args(ty: &Type) -> bool {
    match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .map(|segment| {
                matches!(&segment.arguments, PathArguments::AngleBracketed(args) if !args.args.is_empty())
            })
            .unwrap_or(false),
        _ => false,
    }
}

// Return the ident if the type matches a generic param.
fn generic_param_ident(
    ty: &Type,
    generic_params: &std::collections::HashSet<String>,
) -> Option<syn::Ident> {
    let ident = match ty {
        Type::Path(path) => path
            .path
            .segments
            .last()
            .filter(|segment| segment.arguments.is_empty())
            .map(|segment| segment.ident.clone()),
        _ => None,
    };

    match ident {
        Some(ident) if generic_params.contains(&ident.to_string()) => Some(ident),
        _ => None,
    }
}

// Detect primitive-ish types that should be replace-patched.
fn replaceable_scalar_ident(ty: &Type) -> Option<String> {
    let last_ident = match ty {
        Type::Path(path) if path.qself.is_none() => path
            .path
            .segments
            .last()
            .map(|segment| segment.ident.to_string()),
        _ => None,
    };

    match last_ident.as_deref() {
        Some("String") | Some("str") | Some("bool") | Some("char") | Some("usize") | Some("u8")
        | Some("u16") | Some("u32") | Some("u64") | Some("u128") | Some("isize") | Some("i8")
        | Some("i16") | Some("i32") | Some("i64") | Some("i128") | Some("f32") | Some("f64") => {
            last_ident
        }
        _ => None,
    }
}

// Convert a type path like Foo to FooPatch.
fn patch_type(ty: &Type) -> syn::Result<Type> {
    let type_path = match ty {
        Type::Path(path) => Ok(path),
        _ => Err(syn::Error::new_spanned(
            ty,
            "nested patch fields must be a path type",
        )),
    }?;

    let type_path = match type_path.qself.is_some() {
        true => Err(syn::Error::new_spanned(
            ty,
            "nested patch fields cannot use qualified self types (e.g. <T as Trait>::Assoc); \
use #[patch(patch_type = <T as Trait>::AssocPatch)] to specify the patch type, \
or #[patch(ignore)]",
        )),
        false => Ok(type_path),
    }?;

    let mut path = type_path.path.clone();
    let last = path
        .segments
        .last_mut()
        .ok_or_else(|| syn::Error::new_spanned(ty, "expected a path type"))?;

    last.ident = format_ident!("{}Patch", last.ident);

    Ok(Type::Path(TypePath { qself: None, path }))
}

fn type_display(ty: &Type) -> String {
    ty.to_token_stream().to_string()
}
