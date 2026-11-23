//! Source-level L3 scalar guard, including inactive cfg branches and tests.
//! This is not type checking: macro-generated declarations, glob imports, and
//! type-erased payloads require separate review. Named objects are treated
//! as DTOs unless serde-transparent; custom serializers need review as well.
//! Clippy and the sealed ConnectorAmount output bound remain mandatory.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};
use syn::{
    visit::{self, Visit},
    Fields, GenericArgument, PathArguments, Type, UseTree,
};

type Key = Vec<String>;
const NUMBER: u8 = 1;
const TEXT: u8 = 2;
const SHARED: u8 = 4;

struct Definition {
    scope: Key,
    name: String,
    fields: Vec<Type>,
    alias: bool,
    numeric_wrapper: bool,
    generic_payloads: Vec<bool>,
    location: String,
}

#[derive(Default)]
struct Inventory {
    definitions: BTreeMap<Key, Definition>,
    imports: BTreeMap<Key, Key>,
    contexts: Vec<(Key, Type)>,
    ambiguities: Vec<String>,
}

fn monetary(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    [
        "amount",
        "money",
        "price",
        "balance",
        "majorunit",
        "minorunit",
        "subtotal",
    ]
    .iter()
    .any(|word| name.contains(word))
        || matches!(name.as_str(), "fee" | "fees" | "tax" | "total" | "cost")
}

fn key(scope: &[String], name: &str) -> Key {
    let mut result = scope.to_vec();
    result.push(name.to_owned());
    result
}

fn serde_meta(attrs: &[syn::Attribute]) -> Vec<syn::Meta> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("serde"))
        .filter_map(|attr| {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
            .ok()
        })
        .flatten()
        .collect()
}

fn generic_scalar_payload(ty: &Type, generics: &syn::Generics) -> bool {
    match ty {
        Type::Paren(ty) => generic_scalar_payload(&ty.elem, generics),
        Type::Group(ty) => generic_scalar_payload(&ty.elem, generics),
        Type::Reference(ty) => generic_scalar_payload(&ty.elem, generics),
        Type::Path(ty) => {
            if let Some(ident) = ty.path.get_ident() {
                return generics.type_params().any(|param| param.ident == *ident);
            }
            let Some(segment) = ty.path.segments.last() else {
                return false;
            };
            if !matches!(
                segment.ident.to_string().as_str(),
                "Option" | "Box" | "Secret"
            ) {
                return false;
            }
            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                return args.args.iter().any(|arg| match arg {
                    GenericArgument::Type(ty) => generic_scalar_payload(ty, generics),
                    _ => false,
                });
            }
            false
        }
        _ => false,
    }
}

struct Collector<'a> {
    inventory: &'a mut Inventory,
    scope: Key,
    file: &'a Path,
    block: usize,
}

impl Collector<'_> {
    fn define(
        &mut self,
        name: &syn::Ident,
        fields: Vec<Type>,
        alias: bool,
        numeric_wrapper: bool,
        generics: &syn::Generics,
    ) {
        let symbol = key(&self.scope, &name.to_string());
        if self.inventory.definitions.contains_key(&symbol) {
            self.inventory.ambiguities.push(format!("{}: ambiguous definition `{}` (including inactive cfg alternatives); cannot establish scalar shape", self.file.display(), name));
        }
        self.inventory.definitions.insert(
            symbol,
            Definition {
                scope: self.scope.clone(),
                name: name.to_string(),
                generic_payloads: fields
                    .iter()
                    .map(|field| generic_scalar_payload(field, generics))
                    .collect(),
                fields,
                alias,
                numeric_wrapper,
                location: self.file.display().to_string(),
            },
        );
    }

    fn imports(&mut self, tree: &UseTree, prefix: Key) {
        match tree {
            UseTree::Path(path) => self.imports(&path.tree, key(&prefix, &path.ident.to_string())),
            UseTree::Group(group) => {
                for item in &group.items {
                    self.imports(item, prefix.clone());
                }
            }
            UseTree::Name(name) => {
                let target = if name.ident == "self" {
                    prefix
                } else {
                    key(&prefix, &name.ident.to_string())
                };
                if let Some(name) = target.last() {
                    self.inventory
                        .imports
                        .insert(key(&self.scope, name), target.clone());
                }
            }
            UseTree::Rename(rename) => {
                let target = if rename.ident == "self" {
                    prefix
                } else {
                    key(&prefix, &rename.ident.to_string())
                };
                self.inventory
                    .imports
                    .insert(key(&self.scope, &rename.rename.to_string()), target);
            }
            UseTree::Glob(_) => {}
        }
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.scope.push(item.ident.to_string());
        visit::visit_item_mod(self, item);
        self.scope.pop();
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.block += 1;
        self.scope.push(format!("@{}", self.block));
        visit::visit_block(self, block);
        self.scope.pop();
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.imports(&item.tree, Vec::new());
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        let fields = if matches!(item.ty.as_ref(), Type::Tuple(_)) {
            Vec::new()
        } else {
            vec![(*item.ty).clone()]
        };
        self.define(&item.ident, fields, true, true, &item.generics);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        // A named serde object is a provider envelope, not a scalar newtype.
        let transparent = serde_meta(&item.attrs)
            .iter()
            .any(|meta| meta.path().is_ident("transparent"));
        let scalar = matches!(item.fields, Fields::Unnamed(_)) || transparent;
        let fields = if scalar && item.fields.len() == 1 {
            item.fields.iter().map(|f| f.ty.clone()).collect()
        } else {
            Vec::new()
        };
        self.define(&item.ident, fields, false, true, &item.generics);
        visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        let scalar = item.variants.iter().all(|variant| matches!(&variant.fields, Fields::Unnamed(fields) if fields.unnamed.len() == 1));
        let fields = if scalar {
            item.variants
                .iter()
                .flat_map(|v| v.fields.iter().map(|f| f.ty.clone()))
                .collect()
        } else {
            Vec::new()
        };
        self.define(&item.ident, fields, false, false, &item.generics);
        visit::visit_item_enum(self, item);
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        let renamed_amount = serde_meta(&field.attrs).iter().any(|meta| {
            if let syn::Meta::NameValue(meta) = meta {
                if meta.path.is_ident("rename") || meta.path.is_ident("alias") {
                    if let syn::Expr::Lit(value) = &meta.value {
                        if let syn::Lit::Str(value) = &value.lit {
                            return monetary(&value.value());
                        }
                    }
                }
            }
            false
        });
        if field
            .ident
            .as_ref()
            .is_some_and(|name| monetary(&name.to_string()))
            || renamed_amount
        {
            self.inventory
                .contexts
                .push((self.scope.clone(), field.ty.clone()));
        }
        visit::visit_field(self, field);
    }

    fn visit_item_impl(&mut self, item: &'ast syn::ItemImpl) {
        if item.trait_.as_ref().is_some_and(|(_, path, _)| {
            path.segments
                .last()
                .is_some_and(|s| s.ident == "AmountConvertor")
        }) {
            for member in &item.items {
                if let syn::ImplItem::Type(output) = member {
                    if output.ident == "Output" {
                        self.inventory
                            .contexts
                            .push((self.scope.clone(), output.ty.clone()));
                    }
                }
            }
        }
        visit::visit_item_impl(self, item);
    }
}

impl Inventory {
    fn resolve(&self, scope: &[String], path: &[String], depth: usize) -> Key {
        let Some((first, rest)) = path.split_first() else {
            return Vec::new();
        };
        if depth > 64 {
            return path.to_vec();
        }
        let mut base = scope.to_vec();
        let mut tail = path;
        if first == "crate" {
            base.clear();
            tail = rest;
        } else if first == "self" {
            tail = rest;
        } else if first == "super" {
            while let Some((head, rest)) = tail.split_first() {
                if head != "super" {
                    break;
                }
                base.pop();
                tail = rest;
            }
        } else {
            loop {
                let candidate = key(&base, first);
                if let Some(import) = self.imports.get(&candidate) {
                    let mut target = self.resolve(&base, import, depth + 1);
                    target.extend_from_slice(rest);
                    return target;
                }
                let mut target = base.clone();
                target.extend_from_slice(tail);
                if self.definitions.contains_key(&target) {
                    return target;
                }
                if base.pop().is_none() {
                    return path.to_vec();
                }
            }
        }
        base.extend_from_slice(tail);
        base
    }

    fn scalar(&self, scope: &[String], ty: &Type, seen: &mut BTreeSet<Key>) -> u8 {
        match ty {
            Type::Paren(t) => self.scalar(scope, &t.elem, seen),
            Type::Group(t) => self.scalar(scope, &t.elem, seen),
            Type::Reference(t) => self.scalar(scope, &t.elem, seen),
            Type::Path(t) => {
                let path: Key = t
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                let resolved = self.resolve(scope, &path, 0);
                if let Some(definition) = self.definitions.get(&resolved) {
                    if !seen.insert(resolved.clone()) {
                        return 0;
                    }
                    let mut kinds = 0;
                    for field in &definition.fields {
                        let kind = self.scalar(&definition.scope, field, seen);
                        if kind == 0 {
                            kinds = 0;
                            break;
                        }
                        kinds |= kind;
                    }
                    seen.remove(&resolved);
                    return kinds;
                }
                let Some(name) = resolved.last() else {
                    return 0;
                };
                match name.as_str() {
                    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" | "u8" | "u16" | "u32"
                    | "u64" | "u128" | "usize" | "f32" | "f64" | "Decimal" => NUMBER,
                    "String" | "str" => TEXT,
                    "ConnectorMinorUnit"
                    | "StringMinorUnit"
                    | "StringMajorUnit"
                    | "StringTwoDecimalUnit"
                    | "FloatMajorUnit"
                    | "FloatMajorUnitWithTwoDecimals"
                    | "StringMajorUnitWithTwoDecimals"
                    | "StringOrNumberMinorUnit"
                    | "StringOrNumberMajorUnit" => SHARED,
                    "Option" | "Box" | "Secret" => {
                        if let Some(segment) = t.path.segments.last() {
                            if let PathArguments::AngleBracketed(args) = &segment.arguments {
                                if let Some(GenericArgument::Type(inner)) = args.args.first() {
                                    return self.scalar(scope, inner, seen);
                                }
                            }
                        }
                        0
                    }
                    _ => 0,
                }
            }
            _ => 0,
        }
    }

    fn mark(&self, scope: &[String], ty: &Type, monetary: &mut BTreeSet<Key>) {
        struct References<'a> {
            inventory: &'a Inventory,
            scope: &'a [String],
            monetary: &'a mut BTreeSet<Key>,
        }
        impl<'ast> Visit<'ast> for References<'_> {
            fn visit_type_path(&mut self, ty: &'ast syn::TypePath) {
                let path: Key = ty
                    .path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect();
                let resolved = self.inventory.resolve(self.scope, &path, 0);
                if let Some(definition) = self.inventory.definitions.get(&resolved) {
                    if self.monetary.insert(resolved) {
                        for field in &definition.fields {
                            self.inventory.mark(&definition.scope, field, self.monetary);
                        }
                    }
                }
                visit::visit_type_path(self, ty);
            }
        }
        References {
            inventory: self,
            scope,
            monetary,
        }
        .visit_type(ty);
    }
}

fn scan(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(
        io::stdout().lock(),
        "cargo:rerun-if-changed={}",
        dir.display()
    )?;
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        let path = entry.path();
        if entry.file_type()?.is_symlink() {
            return Err(format!("L3 guard cannot scan symlink {}", path.display()).into());
        }
        if path.is_dir() {
            scan(&path, files)?;
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            writeln!(
                io::stdout().lock(),
                "cargo:rerun-if-changed={}",
                path.display()
            )?;
            files.push(path);
        }
    }
    Ok(())
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut files = Vec::new();
    scan(Path::new("src"), &mut files)?;
    let mut inventory = Inventory::default();
    for file in &files {
        let mut scope: Key = file
            .strip_prefix("src")?
            .with_extension("")
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        if scope
            .last()
            .is_some_and(|s| matches!(s.as_str(), "lib" | "mod"))
        {
            scope.pop();
        }
        let ast = syn::parse_file(&fs::read_to_string(file)?)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        Collector {
            inventory: &mut inventory,
            scope,
            file,
            block: 0,
        }
        .visit_file(&ast);
    }
    let mut monetary_types = BTreeSet::new();
    for (scope, ty) in &inventory.contexts {
        inventory.mark(scope, ty, &mut monetary_types);
    }
    let mut violations = inventory.ambiguities.clone();
    for (key, definition) in &inventory.definitions {
        let mut kinds = 0;
        for field in &definition.fields {
            let kind = inventory.scalar(&definition.scope, field, &mut BTreeSet::new());
            if kind == 0 {
                kinds = 0;
                break;
            }
            kinds |= kind;
        }
        let generic_scalar = definition.generic_payloads.iter().any(|generic| *generic)
            && definition
                .fields
                .iter()
                .zip(&definition.generic_payloads)
                .all(|(field, generic)| {
                    *generic
                        || inventory.scalar(&definition.scope, field, &mut BTreeSet::new()) != 0
                });
        if generic_scalar
            || (definition.numeric_wrapper && kinds & NUMBER != 0)
            || (!definition.fields.is_empty()
                && !(definition.alias && kinds == SHARED)
                && (monetary(&definition.name) || monetary_types.contains(key)))
        {
            violations.push(format!("{}: local scalar amount definition `{}`; use a shared common_utils amount type (typed provider object DTOs are permitted)", definition.location, definition.name));
        }
    }
    if !violations.is_empty() {
        return Err(violations.join("\n").into());
    }
    Ok(())
}

fn main() -> ExitCode {
    if let Err(error) = run() {
        let _ = writeln!(
            io::stderr().lock(),
            "L3 money boundary guard failed:\n{error}"
        );
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
