//! Proves the descriptor-driven `auto_populate` mechanism end-to-end for
//! `os_based_return_url`, without any hand-written field -> message mapping.

use grpc_api_types::auto_populate::PopulateOsBasedReturnUrl;
use grpc_api_types::payments::{
    merchant_authentication_service_create_client_authentication_token_request,
    AuthenticatorClientAuthenticationContext, ClientPlatform,
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
                        os_type: client_platform_code(ClientPlatform::Ios),
                        return_url_map: HashMap::new(),
                    }),
                    ..Default::default()
                },
            ),
        ),
        ..Default::default()
    };

    req.populate_os_based_return_url(OsBasedReturnUrl {
        os_type: client_platform_code(ClientPlatform::Unspecified),
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

    assert_eq!(
        os_based_return_url.os_type,
        client_platform_code(ClientPlatform::Ios)
    );
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

fn client_platform_code(platform: ClientPlatform) -> i32 {
    match platform {
        ClientPlatform::Unspecified => 0,
        ClientPlatform::Ios => 1,
        ClientPlatform::Web => 2,
        ClientPlatform::Android => 3,
    }
}
