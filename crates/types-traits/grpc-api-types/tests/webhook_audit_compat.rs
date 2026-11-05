#![allow(deprecated)] // Deliberately exercise old clients throughout the migration.

use std::collections::HashMap;

use grpc_api_types::payments::{
    EventServiceHandleRequest, HttpMethod, RedirectResponseSecrets, RequestDetails, SecretHeaders,
    WebhookSecrets,
};
use hyperswitch_masking::{masked_serialize, PeekInterface, Secret};
use prost::Message;
use serde_json::{json, Value};

// Frozen pre-migration wire shapes: these stand in for an already deployed client.
#[derive(Clone, PartialEq, Message)]
struct OldSecrets {
    #[prost(string, tag = "1")]
    secret: String,
    #[prost(string, optional, tag = "2")]
    additional_secret: Option<String>,
}

#[derive(Clone, PartialEq, Message)]
struct OldDetails {
    #[prost(enumeration = "HttpMethod", tag = "1")]
    method: i32,
    #[prost(string, optional, tag = "2")]
    uri: Option<String>,
    #[prost(map = "string, string", tag = "3")]
    headers: HashMap<String, String>,
    #[prost(bytes = "vec", tag = "4")]
    body: Vec<u8>,
    #[prost(string, optional, tag = "5")]
    query_params: Option<String>,
}

#[test]
fn existing_binary_clients_round_trip_without_changing_bytes() {
    let old = OldSecrets {
        secret: "synthetic-primary".into(),
        additional_secret: Some("synthetic-extra".into()),
    };
    let bytes = old.encode_to_vec();
    assert_eq!(
        WebhookSecrets::decode(bytes.as_slice())
            .unwrap()
            .encode_to_vec(),
        bytes
    );
    assert_eq!(
        RedirectResponseSecrets::decode(bytes.as_slice())
            .unwrap()
            .encode_to_vec(),
        bytes
    );

    let old = OldDetails {
        method: HttpMethod::Post as i32,
        uri: Some("/callback/%2F?token=synthetic-token".into()),
        headers: HashMap::from([("x-signature".into(), "synthetic-signature".into())]),
        body: vec![0, 255, 128, 13, 10],
        query_params: Some("token=synthetic-token".into()),
    };
    let decoded = RequestDetails::decode(old.encode_to_vec().as_slice()).unwrap();
    assert_eq!(
        OldDetails::decode(decoded.encode_to_vec().as_slice()).unwrap(),
        old
    );
}

#[test]
fn existing_http_json_is_unchanged_but_nested_audit_and_debug_are_masked() {
    // The HTTP extractor deserializes these same generated types with Serde.
    let original = json!({
        "request_details": {
            "method": "HTTP_METHOD_POST",
            "uri": "/callback?token=synthetic-uri",
            "headers": {"x-signature": "synthetic-header"},
            "body": [0, 255, 128, 13, 10],
            "query_params": "token=synthetic-query"
        },
        "webhook_secrets": {"secret": "synthetic-primary", "additional_secret": "synthetic-extra"}
    });
    let request: EventServiceHandleRequest = serde_json::from_value(original.clone()).unwrap();
    let ordinary = serde_json::to_value(&request).unwrap();
    assert_eq!(ordinary["request_details"], original["request_details"]);
    assert_eq!(ordinary["webhook_secrets"], original["webhook_secrets"]);

    let masked = masked_serialize(&request).unwrap();
    assert_eq!(masked["request_details"]["method"], "HTTP_METHOD_POST");
    assert!(masked["request_details"]["headers"]
        .get("x-signature")
        .is_some());
    assert_ne!(
        masked["request_details"]["body"],
        original["request_details"]["body"]
    );
    let debug = format!("{request:?}");
    for marker in [
        "synthetic-uri",
        "synthetic-header",
        "synthetic-query",
        "synthetic-primary",
        "synthetic-extra",
    ] {
        assert!(!masked.to_string().contains(marker));
        assert!(!debug.contains(marker));
    }
}

#[test]
fn migrated_http_fields_accept_plain_strings_and_binary_data_without_legacy_fields() {
    let secrets: WebhookSecrets =
        serde_json::from_value(json!({"signing_secret": "synthetic-new"})).unwrap();
    assert_eq!(
        secrets.signing_secret.as_ref().unwrap().peek(),
        "synthetic-new"
    );
    assert!(secrets.secret.is_empty());
    let details: RequestDetails = serde_json::from_value(json!({
        "method": "HTTP_METHOD_POST",
        "sensitive_body": [0, 255, 128],
        "sensitive_headers": {"values": {"x-signature": "synthetic-new-header"}},
        "sensitive_uri": "/callback",
        "sensitive_query_params": ""
    }))
    .unwrap();
    assert_eq!(
        details.sensitive_body.as_ref().unwrap().peek(),
        &[0, 255, 128]
    );
    let decoded = RequestDetails::decode(details.encode_to_vec().as_slice()).unwrap();
    assert_eq!(decoded.sensitive_body.unwrap().peek(), &[0, 255, 128]);
    assert!(!masked_serialize(&details)
        .unwrap()
        .to_string()
        .contains("synthetic-new-header"));
}

#[test]
fn absent_optional_secrets_stay_absent_and_acknowledgements_are_masked() {
    let secret = WebhookSecrets {
        signing_secret: Some(Secret::new("synthetic-new".into())),
        ..Default::default()
    };
    let value = masked_serialize(&secret).unwrap();
    assert_eq!(value.get("additional_secret"), None);
    assert_eq!(value.get("additional_signing_secret"), None);
    let headers = SecretHeaders {
        values: HashMap::new(),
    };
    assert_eq!(
        serde_json::to_value(headers).unwrap(),
        json!({"values": {}})
    );
    let ack = grpc_api_types::payments::EventAckResponse {
        status_code: 200,
        headers: HashMap::from([("set-cookie".into(), "synthetic-cookie".into())]),
        body: b"synthetic-body".to_vec(),
    };
    let value: Value = masked_serialize(&ack).unwrap();
    assert_eq!(value["status_code"], 200);
    assert!(!value.to_string().contains("synthetic-cookie"));
    assert_ne!(value["body"], json!(ack.body));
    assert!(!format!("{ack:?}").contains("synthetic-cookie"));
}
