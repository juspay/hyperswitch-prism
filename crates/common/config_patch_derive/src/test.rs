#![allow(clippy::expect_used)]
use std::collections::HashSet;

use crate::generics::collect_generic_params;
use syn::Type;

fn used_params(ty: &str, params: &[&str]) -> HashSet<String> {
    let ty: Type = syn::parse_str(ty).expect("type should parse");
    let generic_params = params
        .iter()
        .map(|param| param.to_string())
        .collect::<HashSet<_>>();
    let mut used = HashSet::new();
    collect_generic_params(&ty, &generic_params, &mut used);
    used
}

#[test]
fn collects_from_qself() {
    let used = used_params("<T as Trait>::Assoc", &["T"]);
    assert!(used.contains("T"));
}

#[test]
fn collects_from_trait_object_assoc() {
    let used = used_params("Box<dyn AssocTrait<Assoc = T> + Send>", &["T"]);
    assert!(used.contains("T"));
}

#[test]
fn collects_from_bare_fn() {
    let used = used_params("fn(T, &U) -> V", &["T", "U", "V"]);
    for param in ["T", "U", "V"] {
        assert!(used.contains(param));
    }
}

#[test]
fn collects_from_ptr() {
    let used = used_params("*const T", &["T"]);
    assert!(used.contains("T"));
}
