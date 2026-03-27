use proc_macro::TokenStream;
use syn::parse_macro_input;

mod generics;
mod helper;
mod r#macro;

#[cfg(test)]
mod test;

/// Derive a `*Patch` struct and a `Patch` impl for overrides.
///
/// # What it generates
/// - `StructNamePatch` with all fields optional.
/// - `impl Patch<StructNamePatch> for StructName` with field-by-field apply logic.
/// - Patch structs are `#[serde(default, deny_unknown_fields)]` to reject unknown keys.
///
/// # Attributes
/// - `#[patch(ignore)]` on a field: exclude the field from patching.
/// - `#[patch(patch_type = SomeTypePatch)]` on a field: override the nested patch type.
///
/// # Optional fields
/// - `Option<T>` fields use three-state behavior:
///   missing = no change, null = clear, value = apply nested patch (inserting default when needed)
///   or replace.
///
/// # Limitations
/// - Nested patching supports only `T` or `Option<T>` (single layer).
/// - Unknown generic wrappers like `Result<T, E>` or `Option<Option<T>>` are rejected.
///   Use `#[patch(ignore)]` in those cases.
///
/// # Example
/// ```ignore
/// #[derive(serde::Deserialize, config_patch_derive::Patch)]
/// struct Config {
///     mode: String,
///     log: LogConfig,
/// }
///
/// let mut cfg = Config { /* ... */ };
/// let patch: ConfigPatch = serde_json::from_str(r#"{ "mode": "prod" }"#)?;
/// cfg.apply(patch);
/// ```
#[proc_macro_derive(Patch, attributes(patch))]
pub fn derive_patch(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);

    match r#macro::derive_patch_impl(input) {
        Ok(expanded) => expanded,
        Err(err) => err.to_compile_error().into(),
    }
}
