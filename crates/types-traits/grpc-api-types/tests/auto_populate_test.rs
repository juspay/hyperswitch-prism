//! Proves the descriptor-driven `auto_populate` mechanism end-to-end for
//! `os_based_return_url`, without any hand-written field -> message mapping.

use grpc_api_types::auto_populate::PopulateOsBasedReturnUrl;
use grpc_api_types::payments::{
    merchant_authentication_service_create_client_authentication_token_request,
    AuthenticatorClientAuthenticationContext,
    MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest, OsBasedReturnUrl,
};
use std::collections::HashMap;

#[test]
fn populates_nested_os_based_return_url_map_when_context_exists() {
    let mut req = MerchantAuthenticationServiceCreateClientAuthenticationTokenRequest {
        domain_context: Some(
            merchant_authentication_service_create_client_authentication_token_request::DomainContext::Authenticator(
                AuthenticatorClientAuthenticationContext {
                    os_based_return_url: Some(OsBasedReturnUrl {
                        os_type: "ios".to_string(),
                        return_url_map: HashMap::new(),
                    }),
                    ..Default::default()
                },
            ),
        ),
        ..Default::default()
    };

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: String::new(),
        return_url_map: HashMap::from([
            ("ios".to_string(), "https://ios.example/return".to_string()),
            ("android".to_string(), "com.example.android".to_string()),
            ("web".to_string(), "https://web.example/return".to_string()),
        ]),
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

    assert_eq!(os_based_return_url.os_type, "ios");
    assert_eq!(
        os_based_return_url.return_url_map.get("ios"),
        Some(&"https://ios.example/return".to_string())
    );
    assert_eq!(
        os_based_return_url.return_url_map.get("android"),
        Some(&"com.example.android".to_string())
    );
    assert_eq!(
        os_based_return_url.return_url_map.get("web"),
        Some(&"https://web.example/return".to_string())
    );
}
