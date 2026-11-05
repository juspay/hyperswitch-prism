#![allow(deprecated)]

use std::collections::HashMap;

use domain_types::{connector_types, utils::ForeignTryFrom};
use grpc_api_types::payments::{
    HttpMethod, RedirectResponseSecrets, RequestDetails, SecretHeaders, WebhookSecrets,
};
use hyperswitch_masking::Secret;

#[test]
fn legacy_and_migrated_requests_produce_identical_verification_inputs() {
    let body = vec![0, 255, 128, 13, 10];
    let headers = HashMap::from([("x-signature".to_owned(), "synthetic-signature".to_owned())]);
    let uri = "/callback/%2F?token=synthetic-token".to_owned();
    let query = "token=synthetic-token".to_owned();
    let legacy = RequestDetails {
        method: HttpMethod::Post as i32,
        uri: Some(uri.clone()),
        headers: headers.clone(),
        body: body.clone(),
        query_params: Some(query.clone()),
        ..Default::default()
    };
    let migrated = RequestDetails {
        method: HttpMethod::Post as i32,
        sensitive_body: Some(Secret::new(body.clone())),
        sensitive_headers: Some(SecretHeaders {
            values: headers
                .into_iter()
                .map(|(k, v)| (k, Secret::new(v)))
                .collect(),
        }),
        sensitive_uri: Some(Secret::new(uri)),
        sensitive_query_params: Some(Secret::new(query)),
        ..Default::default()
    };
    let legacy = connector_types::RequestDetails::foreign_try_from(legacy).unwrap();
    let migrated = connector_types::RequestDetails::foreign_try_from(migrated).unwrap();
    assert_eq!(legacy.body, migrated.body);
    assert_eq!(legacy.body, body);
    assert_eq!(legacy.headers, migrated.headers);
    assert_eq!(legacy.uri, migrated.uri);
    assert_eq!(legacy.query_params, migrated.query_params);
    assert!(!format!("{legacy:?} {migrated:?}").contains("synthetic"));
}

#[test]
fn explicitly_empty_new_fields_take_precedence_over_legacy_data() {
    let request = RequestDetails {
        method: HttpMethod::Post as i32,
        body: b"legacy".to_vec(),
        sensitive_body: Some(Secret::new(vec![])),
        headers: HashMap::from([("legacy".into(), "legacy".into())]),
        sensitive_headers: Some(SecretHeaders {
            values: HashMap::new(),
        }),
        uri: Some("legacy".into()),
        sensitive_uri: Some(Secret::new(String::new())),
        query_params: Some("legacy".into()),
        sensitive_query_params: Some(Secret::new(String::new())),
    };
    let request = connector_types::RequestDetails::foreign_try_from(request).unwrap();
    assert!(request.body.is_empty());
    assert!(request.headers.is_empty());
    assert_eq!(request.uri.as_deref(), Some(""));
    assert_eq!(request.query_params.as_deref(), Some(""));
    let secret = connector_types::ConnectorWebhookSecrets::foreign_try_from(WebhookSecrets {
        secret: "legacy".into(),
        signing_secret: Some(Secret::new(String::new())),
        ..Default::default()
    })
    .unwrap();
    assert!(secret.secret.is_empty());
}

#[test]
fn both_secret_types_prefer_new_fields_and_keep_legacy_fallback() {
    let legacy = connector_types::ConnectorWebhookSecrets::foreign_try_from(WebhookSecrets {
        secret: "synthetic-legacy".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(legacy.secret, b"synthetic-legacy");
    let migrated = connector_types::ConnectorWebhookSecrets::foreign_try_from(WebhookSecrets {
        secret: "synthetic-legacy".into(),
        signing_secret: Some(Secret::new("synthetic-new".into())),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(migrated.secret, b"synthetic-new");
    let redirect = connector_types::ConnectorRedirectResponseSecrets::foreign_try_from(
        RedirectResponseSecrets {
            secret: "synthetic-legacy".into(),
            verification_secret: Some(Secret::new("synthetic-new".into())),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(redirect.secret, b"synthetic-new");
    assert!(!format!("{legacy:?} {migrated:?} {redirect:?}").contains("synthetic"));
}
