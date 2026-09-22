//! Proves the descriptor-driven `auto_populate` mechanism end-to-end for
//! `os_based_return_url`, without any hand-written field -> message mapping.
//!
//! Split into two groups:
//! - direct-setter tests, exercising `AuthenticatorClientAuthenticationContext`'s
//!   own generated method in isolation (the safety guarantees the setter
//!   itself is supposed to uphold);
//! - delegation tests, exercising the full nested/oneof path starting from
//!   the actual top-level RPC request type.

use std::collections::HashMap;

use grpc_api_types::auto_populate::PopulateOsBasedReturnUrl;
use grpc_api_types::payments::{
    merchant_authentication_service_create_client_authentication_token_request,
    AuthenticatorClientAuthenticationContext, ClientPlatform,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest, OsBasedReturnUrl,
    PaymentClientAuthenticationContext, PaymentServiceVoidRequest,
};

fn sample_return_url_map() -> HashMap<String, String> {
    HashMap::from([
        ("ios".to_string(), "https://ios.example/return".to_string()),
        ("android".to_string(), "com.example.android".to_string()),
        ("web".to_string(), "https://web.example/return".to_string()),
    ])
}

// ---------------------------------------------------------------------
// Direct setter: AuthenticatorClientAuthenticationContext's own generated
// method, called in isolation.
// ---------------------------------------------------------------------

#[test]
fn direct_setter_fills_map_when_os_type_already_set() {
    // Positive/happy path: the request already carries a concrete os_type
    // (Ios). The generated setter should fill in return_url_map and leave
    // the caller-provided os_type exactly as it was.
    let mut context = AuthenticatorClientAuthenticationContext {
        os_based_return_url: Some(OsBasedReturnUrl {
            os_type: i32::from(ClientPlatform::Ios),
            return_url_map: HashMap::new(),
        }),
        ..Default::default()
    };

    context.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Unspecified),
        return_url_map: sample_return_url_map(),
    });

    let updated = context
        .os_based_return_url
        .expect("os_based_return_url should still be present");
    assert_eq!(updated.os_type, i32::from(ClientPlatform::Ios));
    assert_eq!(updated.return_url_map, sample_return_url_map());
}

#[test]
fn direct_setter_does_not_create_missing_os_based_return_url() {
    // Negative: the caller never set os_based_return_url at all. The setter
    // must not invent one — creating missing context is a business decision
    // this generator explicitly refuses to make on the schema's behalf.
    let mut context = AuthenticatorClientAuthenticationContext {
        os_based_return_url: None,
        ..Default::default()
    };

    context.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Ios),
        return_url_map: sample_return_url_map(),
    });

    assert!(
        context.os_based_return_url.is_none(),
        "populate must not create os_based_return_url when the request didn't send it"
    );
}

#[test]
fn direct_setter_does_not_fill_map_when_os_type_unspecified() {
    // Negative: this is the core safety guarantee the whole feature rests
    // on. If os_type is CLIENT_PLATFORM_UNSPECIFIED, the map must stay
    // untouched — filling it in anyway would mean guessing which OS the
    // caller meant.
    let mut context = AuthenticatorClientAuthenticationContext {
        os_based_return_url: Some(OsBasedReturnUrl {
            os_type: i32::from(ClientPlatform::Unspecified),
            return_url_map: HashMap::new(),
        }),
        ..Default::default()
    };

    context.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Unspecified),
        return_url_map: sample_return_url_map(),
    });

    let unchanged = context
        .os_based_return_url
        .expect("os_based_return_url should still be present");
    assert_eq!(unchanged.os_type, i32::from(ClientPlatform::Unspecified));
    assert!(
        unchanged.return_url_map.is_empty(),
        "return_url_map must stay untouched when os_type is unspecified, got {:?}",
        unchanged.return_url_map
    );
}

#[test]
fn direct_setter_replaces_existing_map_entirely_rather_than_merging() {
    // Documents a real, easy-to-misread behavior: the generated setter does
    // existing.return_url_map = value.return_url_map — a plain replace, not
    // a merge. A stale "ios" entry from a previous call does not survive
    // alongside a new map that no longer mentions it.
    let stale_map = HashMap::from([
        ("ios".to_string(), "https://stale.example/old".to_string()),
        (
            "desktop".to_string(),
            "https://stale.example/desktop".to_string(),
        ),
    ]);
    let mut context = AuthenticatorClientAuthenticationContext {
        os_based_return_url: Some(OsBasedReturnUrl {
            os_type: i32::from(ClientPlatform::Android),
            return_url_map: stale_map,
        }),
        ..Default::default()
    };

    let fresh_map = HashMap::from([("android".to_string(), "com.example.fresh".to_string())]);
    context.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Unspecified),
        return_url_map: fresh_map.clone(),
    });

    let updated = context
        .os_based_return_url
        .expect("os_based_return_url should still be present");
    assert_eq!(
        updated.return_url_map, fresh_map,
        "return_url_map must be fully replaced, not merged with the previous value"
    );
    assert!(
        !updated.return_url_map.contains_key("desktop"),
        "stale keys from before the call must not survive"
    );
}

// ---------------------------------------------------------------------
// Delegation: the actual top-level RPC request type, going through the
// domain_context oneof.
// ---------------------------------------------------------------------

#[test]
fn populates_nested_os_based_return_url_map_when_authenticator_context_exists() {
    // Positive/end-to-end: exercises the full delegation path, not just the
    // direct setter — the top-level RPC request's `domain_context` oneof is
    // set to the `authenticator` variant, so populate must walk through the
    // oneof match and reach AuthenticatorClientAuthenticationContext's field.
    let mut req = MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        domain_context: Some(
            merchant_authentication_service_create_client_authentication_token_request::DomainContext::Authenticator(
                AuthenticatorClientAuthenticationContext {
                    os_based_return_url: Some(OsBasedReturnUrl {
                        os_type: i32::from(ClientPlatform::Ios),
                        return_url_map: HashMap::new(),
                    }),
                    ..Default::default()
                },
            ),
        ),
        ..Default::default()
    };

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Unspecified),
        return_url_map: sample_return_url_map(),
    });

    let Some(
        merchant_authentication_service_create_client_authentication_token_request::DomainContext::Authenticator(
            authenticator,
        ),
    ) = req.domain_context
    else {
        panic!("expected authenticator context");
    };
    let os_based_return_url = authenticator
        .os_based_return_url
        .expect("os_based_return_url should still be present");

    assert_eq!(os_based_return_url.os_type, i32::from(ClientPlatform::Ios));
    assert_eq!(os_based_return_url.return_url_map, sample_return_url_map());
}

#[test]
fn oneof_set_to_non_qualifying_variant_is_a_noop() {
    // Negative: domain_context is set, but to the `payment` branch, which
    // has no path to os_based_return_url at all. The oneof match's
    // wildcard arm must leave it completely untouched, not just skip the
    // map fill.
    let payment_context = PaymentClientAuthenticationContext::default();
    let mut req = MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        domain_context: Some(
            merchant_authentication_service_create_client_authentication_token_request::DomainContext::Payment(
                payment_context.clone(),
            ),
        ),
        ..Default::default()
    };

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Ios),
        return_url_map: sample_return_url_map(),
    });

    let Some(
        merchant_authentication_service_create_client_authentication_token_request::DomainContext::Payment(
            unchanged,
        ),
    ) = req.domain_context
    else {
        panic!("expected the payment variant to still be selected");
    };
    assert_eq!(
        unchanged, payment_context,
        "the payment variant must be left completely unmodified"
    );
}

#[test]
fn oneof_unset_is_a_noop() {
    // Negative: domain_context was never set at all.
    let mut req = MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        domain_context: None,
        ..Default::default()
    };

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Ios),
        return_url_map: sample_return_url_map(),
    });

    assert!(
        req.domain_context.is_none(),
        "populate must not invent a domain_context that was never set"
    );
}

#[test]
fn unrelated_request_with_no_path_to_the_field_is_a_true_noop() {
    // Positive proof of the "no path at all" case: PaymentServiceVoidRequest
    // is a real RPC input type (so it must implement the trait, or the
    // sanity layer's generic dispatch wouldn't compile for the Void RPC),
    // but it has no field or nested path leading to os_based_return_url —
    // so it should get the trait's bare default, with zero observable
    // effect on the request.
    let mut req = PaymentServiceVoidRequest::default();
    let before = req.clone();

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: i32::from(ClientPlatform::Ios),
        return_url_map: sample_return_url_map(),
    });

    assert_eq!(
        req, before,
        "a request with no path to the field must be completely unaffected by populate"
    );
}
