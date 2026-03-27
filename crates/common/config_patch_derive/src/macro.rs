use quote::{format_ident, quote};
use syn::{parse_quote, Attribute, Data, DeriveInput, Fields, Generics};

use crate::generics::{
    add_where_bounds, append_patch_params, build_patch_generics, GenericPatchCtx,
};
use crate::helper::build_patch_field_specific_metadata;

// Build the derive expansion for a struct.
pub(crate) fn derive_patch_impl(input: DeriveInput) -> syn::Result<proc_macro::TokenStream> {
    let DeriveInput {
        ident: struct_name,
        vis,
        data,
        generics,
        ..
    } = input;

    let fields = match data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            Fields::Unnamed(_) | Fields::Unit => {
                return derive_replace_patch_impl(struct_name, vis, generics)
            }
        },
        Data::Enum(_) => return derive_replace_patch_impl(struct_name, vis, generics),
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                struct_name,
                "Patch derive only supports structs or enums you can use #[patch(ignore)] to skip union types",
            ))
        }
    };
    let patch_name = format_ident!("{}Patch", struct_name);
    let mut patch_ctx = GenericPatchCtx::new(&generics);

    let mut patch_fields = Vec::new();
    let mut apply_stmts = Vec::new();

    for field in fields {
        let field_spec = build_patch_field_specific_metadata(&field, &mut patch_ctx)?;

        if let Some(spec) = field_spec {
            let doc_attrs = &spec.doc_attrs;
            let serde_attrs = &spec.serde_attrs;
            let field_ident = &spec.ident;
            let patch_field_ty = &spec.patch_field_ty;
            patch_fields.push(quote! {
                #(#doc_attrs)*
                #(#serde_attrs)*
                pub #field_ident: #patch_field_ty
            });
            apply_stmts.push(spec.apply_stmt);
        }
    }

    let patch_generics = build_patch_generics(
        &generics,
        &patch_ctx.used_type_params,
        &patch_ctx.patch_params,
    );
    let impl_generics_source = add_where_bounds(
        &append_patch_params(&generics, &patch_ctx.patch_params),
        &patch_ctx.where_bounds,
    );
    let (impl_generics, _, where_clause) = impl_generics_source.split_for_impl();
    let (_, patch_ty_generics, _) = patch_generics.split_for_impl();
    let (_, struct_ty_generics, _) = generics.split_for_impl();

    let patch_doc = format!(
        "Generated patch type for `{}`. Missing fields mean no change; optional fields treat null as clear.",
        struct_name
    );
    let patch_doc_attr: Attribute = parse_quote!(#[doc = #patch_doc]);

    Ok(quote! {
        #patch_doc_attr
        #[derive(Debug, Default, ::serde::Serialize, ::serde::Deserialize)]
        #[serde(default, deny_unknown_fields)]
        #vis struct #patch_name #patch_generics {
            #(#patch_fields,)*
        }

        impl #impl_generics ::common_utils::config_patch::Patch<#patch_name #patch_ty_generics>
            for #struct_name #struct_ty_generics #where_clause
        {
            fn apply(&mut self, patch: #patch_name #patch_ty_generics) {
                #(#apply_stmts)*
            }
        }
    }
    .into())
}

fn derive_replace_patch_impl(
    item_name: syn::Ident,
    vis: syn::Visibility,
    generics: Generics,
) -> syn::Result<proc_macro::TokenStream> {
    let patch_name = format_ident!("{}Patch", item_name);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let patch_doc = format!(
        "Generated patch type for `{}`. This type is patched by replacement.",
        item_name
    );
    let patch_doc_attr: Attribute = parse_quote!(#[doc = #patch_doc]);

    Ok(quote! {
        #patch_doc_attr
        #vis type #patch_name #generics = #item_name #ty_generics;

        impl #impl_generics ::common_utils::config_patch::Patch<#patch_name #ty_generics>
            for #item_name #ty_generics #where_clause
        {
            fn apply(&mut self, patch: #patch_name #ty_generics) {
                *self = patch;
            }
        }
    }
    .into())
}
