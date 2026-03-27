use quote::{format_ident, ToTokens};
use std::collections::{HashMap, HashSet};
use syn::{parse_quote, visit::Visit, Type, TypePath, WherePredicate};

pub(crate) struct GenericPatchCtx {
    /// Generic params declared on the source struct.
    pub(crate) generic_params: HashSet<String>,
    /// Patch-specific generic params to add.
    pub(crate) patch_params: Vec<syn::TypeParam>,
    /// Mapping from generic name to patch generic name.
    patch_map: HashMap<String, syn::Ident>,
    /// Trait bounds required by nested patches.
    pub(crate) where_bounds: Vec<WherePredicate>,
    /// Generics that actually appear in patch fields.
    pub(crate) used_type_params: HashSet<String>,
    /// Names reserved to avoid collisions.
    used_names: HashSet<String>,
    /// Dedup key for generated bounds.
    bound_keys: HashSet<String>,
}

impl GenericPatchCtx {
    // Initialize the patch context from the source generics.
    pub(crate) fn new(generics: &syn::Generics) -> Self {
        let generic_params = generics
            .type_params()
            .map(|param| param.ident.to_string())
            .collect::<HashSet<_>>();
        let used_names = generics
            .params
            .iter()
            .map(|param| match param {
                syn::GenericParam::Type(tp) => tp.ident.to_string(),
                syn::GenericParam::Lifetime(lt) => lt.lifetime.ident.to_string(),
                syn::GenericParam::Const(cp) => cp.ident.to_string(),
            })
            .collect::<HashSet<_>>();

        Self {
            generic_params,
            patch_params: Vec::new(),
            patch_map: HashMap::new(),
            where_bounds: Vec::new(),
            used_type_params: HashSet::new(),
            used_names,
            bound_keys: HashSet::new(),
        }
    }

    // Allocate or reuse a patch generic name for a type param.
    pub(crate) fn patch_ident(&mut self, ident: &syn::Ident) -> syn::Ident {
        let key = ident.to_string();
        match self.patch_map.get(&key) {
            Some(existing) => existing.clone(),
            None => {
                let patch_ident = self.unique_patch_ident(&key);
                let patch_param: syn::TypeParam = parse_quote!(#patch_ident);
                self.patch_params.push(patch_param);
                self.patch_map.insert(key, patch_ident.clone());
                patch_ident
            }
        }
    }

    // Add Patch/Default bounds for nested patching.
    pub(crate) fn add_bound(
        &mut self,
        ident: &syn::Ident,
        patch_ident: &syn::Ident,
        needs_default: bool,
    ) {
        let key = format!("{}:{}:{}", ident, patch_ident, needs_default);
        let already_added = self.bound_keys.contains(&key);
        let predicate = match (already_added, needs_default) {
            (true, _) => None,
            (false, true) => Some(parse_quote!(
                #ident: ::common_utils::config_patch::Patch<#patch_ident> + Default
            )),
            (false, false) => Some(parse_quote!(
                #ident: ::common_utils::config_patch::Patch<#patch_ident>
            )),
        };
        if let Some(pred) = predicate {
            self.bound_keys.insert(key);
            self.where_bounds.push(pred);
        }
    }

    // Add bounds for an explicit nested patch type.
    pub(crate) fn add_type_bound(&mut self, ty: &Type, patch_ty: &Type, needs_default: bool) {
        let key = format!(
            "{}:{}:{}",
            ty.to_token_stream(),
            patch_ty.to_token_stream(),
            needs_default
        );
        if !self.bound_keys.contains(&key) {
            let predicate = match needs_default {
                true => parse_quote!(#ty: ::common_utils::config_patch::Patch<#patch_ty> + Default),
                false => parse_quote!(#ty: ::common_utils::config_patch::Patch<#patch_ty>),
            };
            self.bound_keys.insert(key);
            self.where_bounds.push(predicate);
        }
    }

    // Generate a unique patch type parameter name.
    fn unique_patch_ident(&mut self, base: &str) -> syn::Ident {
        let mut candidate = format!("{base}Patch");
        let mut counter = 2;
        while self.used_names.contains(&candidate) {
            candidate = format!("{base}Patch{counter}");
            counter += 1;
        }
        self.used_names.insert(candidate.clone());
        format_ident!("{candidate}")
    }

    // Track generics used by a field type.
    pub(crate) fn record_used_type_params_for_the_field(&mut self, ty: &Type) {
        collect_generic_params(ty, &self.generic_params, &mut self.used_type_params);
    }
}

// Append patch-only generic params to a generics list.
pub(crate) fn append_patch_params(
    generics: &syn::Generics,
    patch_params: &[syn::TypeParam],
) -> syn::Generics {
    let mut next = generics.clone();
    for param in patch_params {
        next.params.push(syn::GenericParam::Type(param.clone()));
    }
    next
}

// Add generated bounds to a where clause.
pub(crate) fn add_where_bounds(
    generics: &syn::Generics,
    bounds: &[WherePredicate],
) -> syn::Generics {
    let mut next = generics.clone();
    match bounds.is_empty() {
        true => next,
        false => {
            let clause = next.where_clause.get_or_insert_with(|| parse_quote!(where));
            for bound in bounds {
                clause.predicates.push(bound.clone());
            }
            next
        }
    }
}

// Keep only used generics and append patch params.
pub(crate) fn build_patch_generics(
    generics: &syn::Generics,
    used_type_params: &HashSet<String>,
    patch_params: &[syn::TypeParam],
) -> syn::Generics {
    let mut next = generics.clone();
    next.params = next
        .params
        .into_iter()
        .filter(|param| match param {
            syn::GenericParam::Type(tp) => used_type_params.contains(&tp.ident.to_string()),
            _ => true,
        })
        .collect();

    for param in patch_params {
        next.params.push(syn::GenericParam::Type(param.clone()));
    }

    next
}

// Walk a type and record any referenced generic params.
pub(crate) fn collect_generic_params(
    ty: &Type,
    generic_params: &HashSet<String>,
    used: &mut HashSet<String>,
) {
    let mut collector = GenericParamCollector {
        generic_params,
        used,
    };
    collector.visit_type(ty);
}

struct GenericParamCollector<'a> {
    generic_params: &'a HashSet<String>,
    used: &'a mut HashSet<String>,
}

impl<'a, 'ast> Visit<'ast> for GenericParamCollector<'a> {
    fn visit_type_path(&mut self, node: &'ast TypePath) {
        for segment in &node.path.segments {
            if segment.arguments.is_empty() {
                let name = segment.ident.to_string();
                if self.generic_params.contains(&name) {
                    self.used.insert(name);
                }
            }
        }
        syn::visit::visit_type_path(self, node);
    }
}
